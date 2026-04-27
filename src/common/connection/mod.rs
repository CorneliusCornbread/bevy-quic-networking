use bevy::{
    ecs::component::Component,
    log::{
        tracing::{self},
        warn,
    },
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Handle as ConnectionHandle};
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
        open_flag::OpenFlag,
        stream_flag::StreamFlag,
        task::{ConnectionCommandError, ConnectionHandleTask, ConnectionTask, ConnectionTaskState},
    },
    stream::{
        QuicBidirectionalStreamAttempt, QuicPeerStream, QuicPeerStreamAttempt,
        QuicReceiveStreamAttempt, QuicSendStreamAttempt, receive::QuicReceiveStream,
        send::QuicSendStream,
    },
};

pub mod disconnect;
pub mod id;
pub(super) mod open_flag;
pub mod plugin;
pub mod runtime;
pub(super) mod stream_flag;
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

#[derive(Debug, Component)]
pub struct QuicConnection {
    runtime: Handle,
    conn_handle: ConnectionHandle,
    task_state: ConnectionTaskState,
    conn_command_channel: mpsc::Sender<ConnectionCommand>,
    is_open: OpenFlag,
    parent_id: QuicParentId,
    /// Flag set by async wakers as soon as there's a new stream
    pending_stream: StreamFlag,
}

impl QuicConnection {
    #[tracing::instrument(
        name = "new_quic_connection"
        skip(runtime),
    )]
    pub fn new(runtime: Handle, mut connection: Connection, parent_id: QuicParentId) -> Self {
        let res = connection.keep_alive(true);
        let (send, rec) = mpsc::channel(CONNECTION_CTRL_CHANNEL_SIZE);

        let pending_stream = StreamFlag::new(false);

        if let Err(e) = res {
            warn!(
                "Unable to mark new connection with keep alive, is the connection already closed? Reason: \"{}\"",
                e
            );
        }

        let is_open = OpenFlag::new(true);
        let conn_handle = connection.handle();
        let task = ConnectionTask::new(
            connection,
            rec,
            parent_id,
            is_open.clone(),
            pending_stream.clone(),
        );

        let handle = runtime.spawn(task.start());

        Self {
            runtime: runtime.clone(),
            conn_handle,
            task_state: ConnectionTaskState::new(runtime, handle),
            conn_command_channel: send,
            is_open,
            parent_id,
            pending_stream,
        }
    }

    /// Accepts any incoming streams, this will always return an [QuicPeerStreamAttempt] even if
    /// there are no pending streams.
    ///
    /// There's also a chance that an accept will successfully get a stream even if there aren't
    /// any pending streams due to network timings.
    ///
    /// Returns an error if the async communication channel errors out due to being full.
    pub fn accept_stream(&mut self) -> Result<QuicPeerStreamAttempt, ConnectionCommandError> {
        self.pending_stream.set_false();

        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::Accept { respond_to: send };
        let send_res = self.conn_command_channel.try_send(cmd);

        if let Err(err) = send_res {
            return Err(err.into());
        }

        let attempt = QuicPeerStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        Ok(attempt)
    }

    /// Accepts incoming receive streams, this will always return an [QuicReceiveStreamAttempt] even if
    /// there are no pending streams.
    ///
    /// There's also a chance that an accept will successfully get a stream even if there aren't
    /// any pending streams due to network timings.
    ///
    /// Returns an error if the async communication channel errors out due to being full.
    pub fn accept_receive_stream(
        &mut self,
    ) -> Result<QuicReceiveStreamAttempt, ConnectionCommandError> {
        self.pending_stream.set_false();

        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::AcceptReceive { respond_to: send };
        let send_res = self.conn_command_channel.try_send(cmd);

        if let Err(err) = send_res {
            return Err(err.into());
        }

        let attempt = QuicReceiveStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        Ok(attempt)
    }

    /// Accepts incoming bidirectional streams, this will always return an [QuicBidirectionalStreamAttempt] even if
    /// there are no pending streams.
    ///
    /// There's also a chance that an accept will successfully get a stream even if there aren't
    /// any pending streams due to network timings.
    ///
    /// Returns an error if the async communication channel errors out due to being full.
    pub fn accept_bidirectional_stream(
        &mut self,
    ) -> Result<QuicBidirectionalStreamAttempt, ConnectionCommandError> {
        self.pending_stream.set_false();

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

    /// Attempts to open a new bidirectional stream to be accepted by the remote peer.
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

    /// Attempts to open a new send stream to be accepted by the remote peer.
    pub fn open_send_stream(&mut self) -> Result<QuicSendStreamAttempt, ConnectionCommandError> {
        let task = ConnectionHandleTask::new(
            self.conn_handle.clone(),
            self.is_open.clone(),
            self.parent_id,
        );

        let join = self.runtime.spawn(task.open_send());

        Ok(QuicSendStreamAttempt::new(
            self.runtime.clone(),
            join,
            self.parent_id,
        ))
    }

    /// Returns true if the connection is still open.
    pub fn is_open(&mut self) -> bool {
        !self.task_state.is_finished() && self.is_open.get()
    }

    /// Returns true of a new stream of any kind is pending.
    pub fn pending_new_stream(&self) -> bool {
        self.pending_stream.get()
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
