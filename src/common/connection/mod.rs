use bevy::{
    ecs::component::Component,
    log::{
        info,
        tracing::{self},
        warn,
    },
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError, stream::PeerStream};
use std::{
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
    task::JoinHandle,
};

use crate::common::{
    QuicParentId,
    attempt::{QuicActionAttempt, TaskError},
    connection::{
        disconnect::{ConnectionDisconnectReason, ConnectionErrorDisconnected},
        task_state::ConnectionTaskState,
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
pub mod task_state;

/// Number of messages that can sit unhandled by the connection task
const CONNECTION_CTRL_CHANNEL_SIZE: usize = 1024;

const CONNECTION_CMD_BUFF_SIZE_MAX: usize = 128;
const CONNECTION_CMD_BUFF_SIZE_MIN: usize = 32;

type ConnectionResponse<T> = Result<Option<T>, TaskError>;

enum ConnectionCommand {
    OpenBidirectional {
        respond_to: oneshot::Sender<ConnectionResponse<(QuicReceiveStream, QuicSendStream)>>,
    },
    OpenSend {
        respond_to: oneshot::Sender<ConnectionResponse<QuicSendStream>>,
    },
    AcceptReceive {
        respond_to: oneshot::Sender<ConnectionResponse<QuicReceiveStream>>,
    },
    AcceptBidirectional {
        respond_to: oneshot::Sender<ConnectionResponse<(QuicReceiveStream, QuicSendStream)>>,
    },
    Accept {
        respond_to: oneshot::Sender<ConnectionResponse<QuicPeerStream>>,
    },
    CloseConnection {
        error_code: s2n_quic::application::Error,
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
    task_state: ConnectionTaskState,
    conn_command_channel: mpsc::Sender<ConnectionCommand>,
    open_flag: Arc<AtomicBool>,
    parent_id: QuicParentId,
    id: u64,
}

impl QuicConnection {
    #[tracing::instrument(
        name = "new_quic_connection"
        skip(runtime),
    )]
    pub fn new(runtime: Handle, mut connection: Connection, parent_id: QuicParentId) -> Self {
        let id = connection.id();
        let res = connection.keep_alive(true);
        let (send, rec) = mpsc::channel(CONNECTION_CTRL_CHANNEL_SIZE);

        if let Err(e) = res {
            warn!(
                "Unable to mark new connection with keep alive, is the connection already closed? Reason: \"{}\"",
                e
            );
        }

        let open_flag = Arc::new(AtomicBool::new(true));
        let task = ConnectionTask::new(connection, rec, parent_id, open_flag.clone());
        let handle = runtime.spawn(task.start());

        Self {
            runtime: runtime.clone(),
            task_state: ConnectionTaskState::new(runtime, handle),
            conn_command_channel: send,
            open_flag,
            parent_id,
            id,
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

    pub fn open_bidrectional_stream(
        &mut self,
    ) -> Result<QuicBidirectionalStreamAttempt, ConnectionCommandError> {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::OpenBidirectional { respond_to: send };
        let send_res = self.conn_command_channel.try_send(cmd);

        if let Err(err) = send_res {
            return Err(err.into());
        }

        let attempt =
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        Ok(attempt)
    }

    /// Returns true if the connection is still open.
    pub fn is_open(&mut self) -> bool {
        !self.task_state.is_finished() & self.open_flag.load(Ordering::Relaxed)
    }

    /// Gets the disconnect reason if the stream has closed.
    /// Returns `None` if the stream is still open.
    pub fn get_disconnect_reason(&mut self) -> Option<ConnectionDisconnectReason> {
        self.task_state.get_disconnect_reason()
    }

    pub fn parent_id(&self) -> QuicParentId {
        self.parent_id
    }

    // TODO: Create type for connection IDs
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Debug)]
struct ConnectionTask {
    connection: Connection,
    cmd_receiver: mpsc::Receiver<ConnectionCommand>,
    disconnect_flag: Option<ConnectionDisconnectReason>,
    is_open: Arc<AtomicBool>,
    parent_id: QuicParentId,
}

impl ConnectionTask {
    fn new(
        connection: Connection,
        cmd_receiver: mpsc::Receiver<ConnectionCommand>,
        parent_id: QuicParentId,
        open_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            connection,
            cmd_receiver,
            disconnect_flag: None,
            is_open: open_flag,
            parent_id,
        }
    }

    #[tracing::instrument(
        name = "quic_connection_task"
        skip(self),
        fields(
            connection_id = self.connection.id(),
            parent_id = %self.parent_id,
            remote_address = ?self.connection.remote_addr()
        )
    )]
    async fn start(mut self) -> ConnectionDisconnectReason {
        info!("New connection opened");

        let mut cmd_buf = Vec::with_capacity(CONNECTION_CMD_BUFF_SIZE_MIN);

        while self.disconnect_flag.is_none() {
            let count = self
                .cmd_receiver
                .recv_many(&mut cmd_buf, CONNECTION_CMD_BUFF_SIZE_MAX)
                .await;

            let mut processed: usize = 0;

            for cmd in cmd_buf.drain(..count) {
                processed += 1;

                // TODO: use handles for these commands while running them asynchronously
                // use join set to make sure they finish successfully
                match cmd {
                    ConnectionCommand::OpenBidirectional { respond_to } => {
                        self.open_bidirectional(respond_to).await;
                    }
                    ConnectionCommand::OpenSend { respond_to } => {
                        self.open_send(respond_to).await;
                    }
                    ConnectionCommand::AcceptReceive { respond_to } => {
                        self.accept_receive(respond_to).await;
                    }
                    ConnectionCommand::AcceptBidirectional { respond_to } => {
                        self.accept_bidirectional(respond_to).await;
                    }
                    ConnectionCommand::Accept { respond_to } => {
                        self.accept(respond_to).await;
                    }
                    ConnectionCommand::CloseConnection { error_code } => {
                        self.connection.close(error_code);

                        if count > processed {
                            warn!(
                                "Connection closed with other commands unprocessed. These commands will be dropped."
                            );
                        }

                        self.disconnect_flag = Some(ConnectionDisconnectReason::UserClosed);
                    }
                }
            }
        }

        self.disconnect_flag
            .unwrap_or(ConnectionDisconnectReason::InternalError(Arc::new(
                MissingErrorData,
            )))
    }

    async fn open_bidirectional(
        &mut self,
        respond_to: oneshot::Sender<ConnectionResponse<(QuicReceiveStream, QuicSendStream)>>,
    ) -> Result<(), ConnectionError> {
        let bidir_res = self.connection.open_bidirectional_stream().await;

        match bidir_res {
            Ok(stream) => {
                let (rec_stream, send_stream) = stream.split();

                let quic_send = QuicSendStream::new(Handle::current(), send_stream, self.parent_id);
                let quic_rec =
                    QuicReceiveStream::new(Handle::current(), rec_stream, self.parent_id);

                let send_err = respond_to.send(Ok(Some((quic_rec, quic_send)))).is_err();

                if send_err {
                    warn!(
                        "Bidrectional stream was opened with the response handler being closed before it could be sent."
                    );
                }
            }
            Err(err) => {
                let send_err = respond_to
                    .send(Err(TaskError::ConnectionFailed(err)))
                    .is_err();

                if send_err {
                    warn!(
                        "Opened bidrectional stream errored with the response handler being closed before it could be sent: {0}",
                        err
                    );
                }

                return Err(err);
            }
        };

        Ok(())
    }

    async fn open_send(
        &mut self,
        respond_to: oneshot::Sender<ConnectionResponse<QuicSendStream>>,
    ) -> Result<(), ConnectionError> {
        let send_res = self.connection.open_send_stream().await;

        match send_res {
            Ok(stream) => {
                let quic_send = QuicSendStream::new(Handle::current(), stream, self.parent_id);
                let send_err = respond_to.send(Ok(Some(quic_send))).is_err();

                if send_err {
                    warn!(
                        "Send stream was opened with the response handler being closed before it could be sent."
                    );
                }
            }
            Err(err) => {
                let send_err = respond_to
                    .send(Err(TaskError::ConnectionFailed(err)))
                    .is_err();

                if send_err {
                    warn!(
                        "Opened send stream errored with the response handler being closed before it could be sent: {0}",
                        err
                    );
                }

                return Err(err);
            }
        }

        Ok(())
    }

    async fn accept_receive(
        &mut self,
        respond_to: oneshot::Sender<ConnectionResponse<QuicReceiveStream>>,
    ) -> Result<(), ConnectionError> {
        // TODO: this will block until we get a stream
        // we need a way to avoid blocking our async task
        let accept_res = self.connection.accept_receive_stream().await;

        match accept_res {
            Ok(rec_opt) => {
                let mapped = rec_opt.map(|rec_stream| {
                    QuicReceiveStream::new(Handle::current(), rec_stream, self.parent_id)
                });

                let send_err = respond_to.send(Ok(mapped)).is_err();

                if send_err {
                    warn!(
                        "Accept stream opened with the response handler being closed before it could be sent.",
                    );
                }
            }
            Err(err) => {
                let send_err = respond_to
                    .send(Err(TaskError::ConnectionFailed(err)))
                    .is_err();

                if send_err {
                    warn!(
                        "Opened send stream errored with the response handler being closed before it could be sent: {0}",
                        err
                    );
                }

                if err.is_closed() {
                    self.is_open.store(false, Ordering::Relaxed);
                }

                return Err(err);
            }
        }

        Ok(())
    }

    async fn accept_bidirectional(
        &mut self,
        respond_to: oneshot::Sender<ConnectionResponse<(QuicReceiveStream, QuicSendStream)>>,
    ) -> Result<(), ConnectionError> {
        let accept_res = self.connection.accept_bidirectional_stream().await;

        match accept_res {
            Ok(rec_opt) => {
                let mapped = rec_opt.map(|bidir_stream| {
                    let (rec, send) = bidir_stream.split();

                    let quic_rec = QuicReceiveStream::new(Handle::current(), rec, self.parent_id);
                    let quic_send = QuicSendStream::new(Handle::current(), send, self.parent_id);

                    (quic_rec, quic_send)
                });

                let send_err = respond_to.send(Ok(mapped)).is_err();

                if send_err {
                    warn!(
                        "Bidrectional stream opened with the response handler being closed before it could be sent.",
                    );
                }
            }
            Err(err) => {
                let send_err = respond_to
                    .send(Err(TaskError::ConnectionFailed(err)))
                    .is_err();

                if send_err {
                    warn!(
                        "Opened bidirectional stream errored with the response handler being closed before it could be sent: {0}",
                        err
                    );
                }

                if err.is_closed() {
                    self.is_open.store(false, Ordering::Relaxed);
                }

                return Err(err);
            }
        }

        Ok(())
    }

    async fn accept(
        &mut self,
        respond_to: oneshot::Sender<ConnectionResponse<QuicPeerStream>>,
    ) -> Result<(), ConnectionError> {
        // TODO: this will block until we get a stream
        // we need a way to avoid blocking our async task
        let accept_res = self.connection.accept().await;

        match accept_res {
            Ok(rec_opt) => {
                let mapped = rec_opt.map(|rec_stream| {
                    QuicPeerStream::new(Handle::current(), rec_stream, self.parent_id)
                });

                let send_err = respond_to.send(Ok(mapped)).is_err();

                if send_err {
                    warn!(
                        "Accept stream opened with the response handler being closed before it could be sent.",
                    );
                }
            }
            Err(err) => {
                let send_err = respond_to
                    .send(Err(TaskError::ConnectionFailed(err)))
                    .is_err();

                if send_err {
                    warn!(
                        "Opened send stream errored with the response handler being closed before it could be sent: {0}",
                        err
                    );
                }

                if err.is_closed() {
                    self.is_open.store(false, Ordering::Relaxed);
                }

                return Err(err);
            }
        }

        Ok(())
    }
}

/// Errors that arise when communicaitons with the async connection task fail.
#[derive(Debug, Error, Clone, Copy)]
pub enum ConnectionCommandError {
    /// The communication channel for the async task is full.
    #[error("The communication channel for the async connection task is full.")]
    Full,
    /// The communication channel for the async task has been closed.
    ///
    /// This is likely due to the async task for the connection quitting unexpectedly.
    #[error("The communication channel for the async connection task has been closed.")]
    Closed,
}

impl<T> From<TrySendError<T>> for ConnectionCommandError {
    fn from(value: TrySendError<T>) -> Self {
        match value {
            TrySendError::Full(_) => Self::Full,
            TrySendError::Closed(_) => Self::Closed,
        }
    }
}

#[derive(Debug)]
pub struct MissingErrorData;

impl fmt::Display for MissingErrorData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Connection task exited without a given reason. This is a bug!"
        )
    }
}

impl Error for MissingErrorData {}
