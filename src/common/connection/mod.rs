use bevy::{
    ecs::component::Component,
    log::{
        tracing::{self},
        warn,
    },
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Handle as ConnectionHandle};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{self},
        oneshot,
    },
    task::JoinHandle,
};

use crate::common::{
    QuicParentId,
    attempt::{QuicActionAttempt, TaskError},
    connection::{
        disconnect::ConnectionDisconnectReason,
        task::{ConnectionCommandError, ConnectionHandleTask, ConnectionTask, ConnectionTaskState},
    },
    stream::{
        QuicBidirectionalStreamAttempt, QuicPeerStream, QuicPeerStreamAttempt,
        QuicReceiveStreamAttempt, receive::QuicReceiveStream, send::QuicSendStream,
    },
};

pub mod disconnect;
pub mod id;
pub mod plugin;
pub mod runtime;
pub mod task;

/// Number of messages that can sit unhandled by the connection task
const CONNECTION_CTRL_CHANNEL_SIZE: usize = 1024;

type ConnectionResponse<T> = Result<Option<T>, TaskError>;

pub(crate) enum ConnectionCommand {
    AcceptReceive {
        respond_to: oneshot::Sender<ConnectionResponse<QuicReceiveStream>>,
    },
    AcceptBidirectional {
        respond_to: oneshot::Sender<ConnectionResponse<(QuicReceiveStream, QuicSendStream)>>,
    },
    Accept {
        respond_to: oneshot::Sender<ConnectionResponse<QuicPeerStream>>,
    },
}

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
pub struct QuicConnectionAttempt(QuicActionAttempt<Connection>);

impl QuicConnectionAttempt {
    pub(crate) fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<Connection, TaskError>>,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task, parent_id))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PendingStreams(Arc<AtomicUsize>);

impl PendingStreams {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn increment(&self) {
        self.0.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn decrement(&self) {
        #[cfg(debug_assertions)]
        {
            let prev = self.0.fetch_sub(1, Ordering::Release);
            if prev == 0 {
                bevy::log::error!("TaskCounter underflowed!");
                // Correct the underflow
                self.0.fetch_add(1, Ordering::Release);
            }
        }

        #[cfg(not(debug_assertions))]
        self.0.fetch_sub(1, Ordering::Release);
    }

    pub(crate) fn count(&self) -> usize {
        self.0.load(Ordering::Acquire)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count() == 0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OpenFlag(Arc<AtomicBool>);

impl OpenFlag {
    pub(crate) fn new(value: bool) -> Self {
        Self(Arc::new(AtomicBool::new(value)))
    }

    pub(crate) fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn set_closed(&self) {
        self.0.store(false, Ordering::Relaxed)
    }

    pub(crate) fn set_open(&self) {
        self.0.store(true, Ordering::Relaxed)
    }
}

#[derive(Debug, Component)]
pub struct QuicConnection {
    runtime: Handle,
    conn_handle: ConnectionHandle,
    task_state: ConnectionTaskState,
    conn_command_channel: mpsc::Sender<ConnectionCommand>,
    is_open: OpenFlag,
    parent_id: QuicParentId,
}

impl QuicConnection {
    #[tracing::instrument(
        name = "new_quic_connection"
        skip(runtime),
    )]
    pub fn new(runtime: Handle, mut connection: Connection, parent_id: QuicParentId) -> Self {
        let res = connection.keep_alive(true);
        let (send, rec) = mpsc::channel(CONNECTION_CTRL_CHANNEL_SIZE);

        if let Err(e) = res {
            warn!(
                "Unable to mark new connection with keep alive, is the connection already closed? Reason: \"{}\"",
                e
            );
        }

        let is_open = OpenFlag::new(true);
        let conn_handle = connection.handle();
        let task = ConnectionTask::new(connection, rec, parent_id, is_open.clone());

        let handle = runtime.spawn(task.start());

        Self {
            runtime: runtime.clone(),
            conn_handle,
            task_state: ConnectionTaskState::new(runtime, handle),
            conn_command_channel: send,
            is_open,
            parent_id,
        }
    }

    pub fn accept_stream(&mut self) -> Result<QuicPeerStreamAttempt, ConnectionCommandError> {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::Accept { respond_to: send };
        let send_res = self.conn_command_channel.try_send(cmd);

        if let Err(err) = send_res {
            return Err(err.into());
        }

        let attempt = QuicPeerStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        Ok(attempt)
    }

    pub fn accept_receive_stream(
        &mut self,
    ) -> Result<QuicReceiveStreamAttempt, ConnectionCommandError> {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::AcceptReceive { respond_to: send };
        let send_res = self.conn_command_channel.try_send(cmd);

        if let Err(err) = send_res {
            return Err(err.into());
        }

        let attempt = QuicReceiveStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        Ok(attempt)
    }

    pub fn accept_bidirectional_stream(
        &mut self,
    ) -> Result<QuicBidirectionalStreamAttempt, ConnectionCommandError> {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::AcceptBidirectional { respond_to: send };
        let send_res = self.conn_command_channel.try_send(cmd);

        if let Err(err) = send_res {
            return Err(err.into());
        }

        let attempt =
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        Ok(attempt)
    }

    pub fn open_bidrectional_stream(
        &mut self,
    ) -> Result<QuicBidirectionalStreamAttempt, ConnectionCommandError> {
        let task = ConnectionHandleTask::new(
            self.conn_handle.clone(),
            self.is_open.clone(),
            self.parent_id,
        );

        let join = self.runtime.spawn(task.open_bidirectional());

        Ok(QuicBidirectionalStreamAttempt::new(
            self.runtime.clone(),
            join,
            self.parent_id,
        ))
    }

    /// Returns true if the connection is still open.
    pub fn is_open(&mut self) -> bool {
        !self.task_state.is_finished() && self.is_open.get()
    }

    /// Gets the disconnect reason if the stream has closed.
    /// Returns `None` if the stream is still open.
    pub fn get_disconnect_reason(&mut self) -> Option<ConnectionDisconnectReason> {
        self.task_state.get_disconnect_reason()
    }

    /// Gets the Id information for the parent client or server for this connection
    pub fn parent_id(&self) -> QuicParentId {
        self.parent_id
    }

    // TODO: Create type for connection IDs
    pub fn id(&self) -> u64 {
        self.conn_handle.id()
    }
}
