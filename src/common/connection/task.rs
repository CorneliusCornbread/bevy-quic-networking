use bevy::log::{
    info,
    tracing::{self},
    warn,
};
use s2n_quic::{
    Connection,
    connection::{Error as ConnectionError, Handle as ConnectionHandle},
};
use std::{error::Error, fmt, net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
};

use crate::common::{
    QuicParentId,
    attempt::TaskError,
    connection::{
        ConnectionCommand, ConnectionResponse, OpenFlag, PendingStreams,
        disconnect::{ConnectionDisconnectReason, ConnectionErrorDisconnected},
    },
    stream::{QuicPeerStream, receive::QuicReceiveStream, send::QuicSendStream},
    task_state::QuicTaskState,
};

const CONNECTION_CMD_BUFF_SIZE_MAX: usize = 128;
const CONNECTION_CMD_BUFF_SIZE_MIN: usize = 32;

pub(in crate::common::connection) type ConnectionTaskState =
    QuicTaskState<ConnectionDisconnectReason>;

#[derive(Debug)]
pub(crate) struct ConnectionHandleTask {
    connection: ConnectionHandle,
    is_open: OpenFlag,
    remote_addr: Result<SocketAddr, ConnectionError>,
    parent_id: QuicParentId,
}

impl fmt::Display for ConnectionHandleTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConnectionHandleTask(parent_id: {}, remote: {:?}, open: {})",
            self.parent_id,
            self.remote_addr,
            self.is_open.get(),
        )
    }
}

impl ConnectionHandleTask {
    pub(crate) fn new(
        connection: ConnectionHandle,
        is_open: OpenFlag,
        parent_id: QuicParentId,
    ) -> Self {
        let remote_addr = connection.remote_addr();

        Self {
            connection,
            is_open,
            remote_addr,
            parent_id,
        }
    }

    /// This always will return Some in the Ok case, this is done to allow accept and open to have the same functionality
    #[tracing::instrument]
    pub(crate) async fn open_bidirectional(
        mut self,
    ) -> Result<Option<(QuicReceiveStream, QuicSendStream)>, TaskError> {
        let bidir_res = self.connection.open_bidirectional_stream().await;

        match bidir_res {
            Ok(stream) => {
                let (rec_stream, send_stream) = stream.split();

                let quic_send = QuicSendStream::new(Handle::current(), send_stream, self.parent_id);
                let quic_rec =
                    QuicReceiveStream::new(Handle::current(), rec_stream, self.parent_id);

                Ok(Some((quic_rec, quic_send)))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// This always will return Some in the Ok case, this is done to allow accept and open to have the same functionality
    #[tracing::instrument]
    pub(crate) async fn open_send(&mut self) -> Result<Option<QuicSendStream>, TaskError> {
        let send_res = self.connection.open_send_stream().await;

        match send_res {
            Ok(stream) => {
                let quic_send = QuicSendStream::new(Handle::current(), stream, self.parent_id);
                Ok(Some(quic_send))
            }
            Err(err) => Err(err.into()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConnectionTask {
    connection: Connection,
    cmd_receiver: mpsc::Receiver<ConnectionCommand>,
    disconnect_flag: Option<ConnectionDisconnectReason>,
    is_open: OpenFlag,
    parent_id: QuicParentId,
}

impl ConnectionTask {
    pub(crate) fn new(
        connection: Connection,
        cmd_receiver: mpsc::Receiver<ConnectionCommand>,
        parent_id: QuicParentId,
        is_open: OpenFlag,
    ) -> Self {
        Self {
            connection,
            cmd_receiver,
            disconnect_flag: None,
            is_open,
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
    pub(crate) async fn start(mut self) -> ConnectionDisconnectReason {
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

                match cmd {
                    ConnectionCommand::AcceptReceive { respond_to } => {
                        self.accept_receive(respond_to).await;
                    }
                    ConnectionCommand::AcceptBidirectional { respond_to } => {
                        self.accept_bidirectional(respond_to).await;
                    }
                    ConnectionCommand::Accept { respond_to } => {
                        self.accept(respond_to).await;
                    }
                }
            }
        }

        self.disconnect_flag
            .unwrap_or(ConnectionDisconnectReason::InternalError(Arc::new(
                MissingErrorData,
            )))
    }

    pub(crate) async fn accept_receive(
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
                    self.is_open.set_closed();
                }

                return Err(err);
            }
        }

        Ok(())
    }

    pub(crate) async fn accept_bidirectional(
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
                    self.is_open.set_closed();
                }

                return Err(err);
            }
        }

        Ok(())
    }

    pub(crate) async fn accept(
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
                    self.is_open.set_closed();
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
