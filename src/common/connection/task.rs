use bevy::log::{
    info,
    tracing::{self},
    warn,
};
use futures::task::waker_ref;
use s2n_quic::{
    Connection,
    connection::{Error as ConnectionError, Handle as ConnectionHandle},
    stream::PeerStream,
};
use std::{
    error::Error,
    fmt,
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
    time::timeout,
};

use crate::common::{
    QuicParentId,
    attempt::TaskError,
    connection::{
        ConnectionCommand, ConnectionResponse,
        disconnect::{ConnectionDisconnectReason, ConnectionErrorDisconnected},
        open_flag::OpenFlag,
        stream_flag::StreamFlag,
    },
    stream::{QuicPeerStream, receive::QuicReceiveStream, send::QuicSendStream},
    task_state::QuicTaskState,
};

const CONNECTION_CMD_BUFF_SIZE_MAX: usize = 128;
const CONNECTION_CMD_BUFF_SIZE_MIN: usize = 32;

/// Timeout used by receive and bidrectional accept variants.
/// If no stream is accepted by this timeout we will assume there are
/// no pending streams and return None
///
/// Regular accept uses poll behaviour so it is not subject to timeouts.
/// This also means the accept() variant is less tolerant to network
/// timings.
const ACCEPT_TIMEOUT: Duration = Duration::from_millis(1);

pub(in crate::common::connection) type ConnectionTaskState =
    QuicTaskState<ConnectionDisconnectReason>;

// TODO: This could be made public and used elsewhere as a async way to open new connections
// or get information about a connection
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
    pub(super) fn new(
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

    /// This always will return Some in the Ok case,
    /// this is done to allow accept and open to have the same functionality
    #[tracing::instrument]
    pub(crate) async fn open_bidirectional(
        mut self,
    ) -> Result<Option<(QuicReceiveStream, QuicSendStream)>, TaskError> {
        let bidir_res = self.connection.open_bidirectional_stream().await;

        match bidir_res {
            Ok(stream) => {
                let (rec_stream, send_stream) = stream.split();

                let quic_send =
                    QuicSendStream::new(Handle::current(), send_stream, self.parent_id);
                let quic_rec =
                    QuicReceiveStream::new(Handle::current(), rec_stream, self.parent_id);

                Ok(Some((quic_rec, quic_send)))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// This always will return Some in the Ok case,
    /// this is done to allow accept and open to have the same functionality
    #[tracing::instrument]
    pub(crate) async fn open_send(mut self) -> Result<Option<QuicSendStream>, TaskError> {
        let send_res = self.connection.open_send_stream().await;

        match send_res {
            Ok(stream) => {
                let quic_send =
                    QuicSendStream::new(Handle::current(), stream, self.parent_id);
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
    pending_stream: Arc<StreamFlag>,
    parent_id: QuicParentId,
    buffered_stream: Option<PeerStream>,
}

impl ConnectionTask {
    pub(crate) fn new(
        connection: Connection,
        cmd_receiver: mpsc::Receiver<ConnectionCommand>,
        parent_id: QuicParentId,
        is_open: OpenFlag,
        pending_stream: Arc<StreamFlag>,
    ) -> Self {
        Self {
            connection,
            cmd_receiver,
            disconnect_flag: None,
            is_open,
            pending_stream,
            parent_id,
            buffered_stream: None,
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

        // We need to run accept() once to make sure the poll is registered
        {
            let waker = waker_ref(&self.pending_stream);
            let mut cx = Context::from_waker(&waker);
            let poll = self.connection.poll_accept(&mut cx);

            // If we actually accept a stream during this buffer it to be handled later
            if let Poll::Ready(Ok(Some(stream))) = poll {
                self.buffered_stream = Some(stream);
            }
        }

        let mut cmd_buf = Vec::with_capacity(CONNECTION_CMD_BUFF_SIZE_MIN);

        while self.disconnect_flag.is_none() {
            let count = self
                .cmd_receiver
                .recv_many(&mut cmd_buf, CONNECTION_CMD_BUFF_SIZE_MAX)
                .await;

            for cmd in cmd_buf.drain(..count) {
                let cmd_res = match cmd {
                    ConnectionCommand::AcceptReceive { respond_to } => {
                        if let Some(PeerStream::Receive(stream)) =
                            self.buffered_stream.take()
                        {
                            let rec_stream = QuicReceiveStream::new(
                                Handle::current(),
                                stream,
                                self.parent_id,
                            );

                            let send_res = respond_to.send(Ok(Some(rec_stream)));

                            if send_res.is_err() {
                                warn!(
                                    "Attempted to send buffered stream resulting in error, connection will be closed."
                                );
                            }

                            Ok(())
                        } else {
                            self.accept_receive(respond_to).await
                        }
                    }
                    ConnectionCommand::AcceptBidirectional { respond_to } => {
                        if let Some(PeerStream::Bidirectional(stream)) =
                            self.buffered_stream.take()
                        {
                            let (rec, send) = stream.split();

                            let rec_stream = QuicReceiveStream::new(
                                Handle::current(),
                                rec,
                                self.parent_id,
                            );

                            let send_stream = QuicSendStream::new(
                                Handle::current(),
                                send,
                                self.parent_id,
                            );

                            let send_res =
                                respond_to.send(Ok(Some((rec_stream, send_stream))));

                            if send_res.is_err() {
                                warn!(
                                    "Attempted to send buffered stream resulting in error, connection will be closed."
                                );
                            }

                            Ok(())
                        } else {
                            self.accept_bidirectional(respond_to).await
                        }
                    }
                    ConnectionCommand::Accept { respond_to } => {
                        if let Some(stream) = self.buffered_stream.take() {
                            let peer_stream = QuicPeerStream::new(
                                Handle::current(),
                                stream,
                                self.parent_id,
                            );

                            let send_res = respond_to.send(Ok(Some(peer_stream)));

                            if send_res.is_err() {
                                warn!(
                                    "Attempted to send buffered stream resulting in error, connection will be closed."
                                );
                            }

                            Ok(())
                        } else {
                            self.accept(respond_to).await
                        }
                    }
                };

                // TODO: make handle result function
                //self.handle_cmd_result(cmd_res);
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
        let timeout =
            timeout(ACCEPT_TIMEOUT, self.connection.accept_receive_stream()).await;

        let Ok(accept_res) = timeout else {
            return Ok(());
        };

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
        respond_to: oneshot::Sender<
            ConnectionResponse<(QuicReceiveStream, QuicSendStream)>,
        >,
    ) -> Result<(), ConnectionError> {
        let timeout = timeout(
            ACCEPT_TIMEOUT,
            self.connection.accept_bidirectional_stream(),
        )
        .await;

        let Ok(accept_res) = timeout else {
            return Ok(());
        };

        match accept_res {
            Ok(rec_opt) => {
                let mapped = rec_opt.map(|bidir_stream| {
                    let (rec, send) = bidir_stream.split();

                    let quic_rec =
                        QuicReceiveStream::new(Handle::current(), rec, self.parent_id);
                    let quic_send =
                        QuicSendStream::new(Handle::current(), send, self.parent_id);

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
        let poll;

        {
            let waker = waker_ref(&self.pending_stream);
            let mut cx = Context::from_waker(&waker);
            poll = self.connection.poll_accept(&mut cx);
        }

        let Poll::Ready(accept_res) = poll else {
            return Ok(());
        };

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
