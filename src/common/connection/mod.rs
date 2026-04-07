use bevy::{
    log::{tracing::Instrument, warn},
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError, stream::PeerStream};
use std::{error::Error, fmt, sync::Arc};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::common::{
    QuicParentId,
    attempt::{QuicActionAttempt, TaskError},
    connection::{disconnect::ConnectionDisconnectReason, task_state::ConnectionTaskState},
    stream::{
        QuicBidirectionalStreamAttempt, QuicReceiveStreamAttempt, receive::QuicReceiveStream,
        send::QuicSendStream,
    },
};

pub mod disconnect;
pub mod id;
pub mod plugin;
pub mod runtime;
pub mod task_state;

/// Number of messages that can sit unhandled by the connection task
const CONNECTION_CTRL_CHANNEL_SIZE: usize = 1024;
/// Number of accepted but Bevy-level pending peer streams
const NEW_STREAM_CHANNEL_SIZE: usize = 32;

const CONNECTION_CMD_BUFF_SIZE_MAX: usize = 128;
const CONNECTION_CMD_BUFF_SIZE_MIN: usize = 32;

enum ConnectionCommand {
    OpenBidirectional {
        respond_to: oneshot::Sender<Result<(QuicReceiveStream, QuicSendStream), TaskError>>,
    },
    OpenSend {
        respond_to: oneshot::Sender<Result<QuicSendStream, TaskError>>,
    },
    AcceptReceive {
        respond_to: oneshot::Sender<Result<Option<QuicReceiveStream>, TaskError>>,
    },
    Accept {
        respond_to: oneshot::Sender<Result<Option<PeerStream>, TaskError>>,
    },
    CloseConnection {
        error_code: s2n_quic::application::Error,
    },
}

#[derive(Deref, DerefMut)]
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

#[derive(Debug)]
pub struct QuicConnection {
    runtime: Handle,
    task_state: ConnectionTaskState,
    conn_command_channel: mpsc::Sender<ConnectionCommand>,
    new_streams: mpsc::Receiver<PeerStream>,
    parent_id: QuicParentId,
    id: u64,
}

impl QuicConnection {
    pub(crate) fn new(
        runtime: Handle,
        mut connection: Connection,
        parent_id: QuicParentId,
    ) -> Self {
        let id = connection.id();

        let res = connection.keep_alive(true);

        let (send, rec) = mpsc::channel(CONNECTION_CTRL_CHANNEL_SIZE);
        let (new_stream_send, new_stream_rec) = mpsc::channel(NEW_STREAM_CHANNEL_SIZE);

        if let Err(e) = res {
            warn!(
                "Unable to mark new connection with keep alive, is the connection already closed? Reason: \"{}\"",
                e
            );
        }

        let span = bevy::log::info_span!(
            "quic_connection_task",
            parent_id = %parent_id,
            connection_id = connection.id(),
        );

        let task = ConnectionTask {
            connection,
            cmd_receiver: rec,
            new_stream_send,
            disconnect_flag: None,
            parent_id,
        };

        let handle = runtime.spawn(task.start().instrument(span));

        Self {
            runtime: runtime.clone(),
            task_state: ConnectionTaskState::new(runtime, handle),
            conn_command_channel: send,
            new_streams: new_stream_rec,
            parent_id,
            id,
        }
    }

    // TODO: make the return type for this more sane
    pub(crate) fn accept_streams(&mut self) -> Result<(PeerStream, QuicParentId), StreamPollError> {
        todo!()
    }

    pub(crate) fn accept_receive_stream(
        &mut self,
    ) -> Result<QuicReceiveStreamAttempt, StreamPollError> {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::AcceptReceive { respond_to: send };

        self.conn_command_channel.blocking_send(cmd);

        let stream_res = rec
            .blocking_recv()
            .map_err(|e| StreamPollError::Error(TaskError::TaskFailed(Arc::new(e))))?;

        let stream: Result<Option<QuicReceiveStream>, StreamPollError> =
            stream_res.map_err(|e| StreamPollError::Error(e));

        //let ReceiveSessionAttempt::
        todo!();
    }

    pub(crate) fn open_bidrectional_stream(&mut self) -> QuicBidirectionalStreamAttempt {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::OpenBidirectional { respond_to: send };

        let attempt =
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), rec, self.parent_id);

        // Just ignore channel errors, they'll get handled in the attempt regardless
        let _send_res = self.conn_command_channel.blocking_send(cmd);

        attempt
    }

    /// Returns true if the connection is still open.
    pub fn is_open(&mut self) -> bool {
        todo!()

        /* let res = self.connection.try_lock();

        match res {
            Ok(mut lock) => lock.ping().is_ok(),
            Err(_e) => true, // If our lock is busy... eh... just assume we're open.
        } */
    }

    /// Gets the disconnect reason if the stream has closed.
    /// Returns `None` if the stream is still open.
    pub fn get_disconnect_reason(&mut self) -> Option<ConnectionError> {
        todo!();

        /* let res = self.connection.try_lock();

        if let Err(_e) = res {
            return None;
        }

        res.unwrap().ping().err() */
    }
}

struct ConnectionTask {
    connection: Connection,
    cmd_receiver: mpsc::Receiver<ConnectionCommand>,
    new_stream_send: mpsc::Sender<PeerStream>,
    disconnect_flag: Option<ConnectionDisconnectReason>,
    parent_id: QuicParentId,
}

impl ConnectionTask {
    pub(crate) async fn start(mut self) -> ConnectionDisconnectReason {
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
                    ConnectionCommand::OpenBidirectional { respond_to } => {
                        let bidir_res = self.connection.open_bidirectional_stream().await;

                        match bidir_res {
                            Ok(stream) => {
                                let (rec_stream, send_stream) = stream.split();

                                let quic_send = QuicSendStream::new(
                                    Handle::current(),
                                    send_stream,
                                    self.parent_id,
                                );
                                let quic_rec = QuicReceiveStream::new(
                                    Handle::current(),
                                    rec_stream,
                                    self.parent_id,
                                );

                                let send_err = respond_to.send(Ok((quic_rec, quic_send))).is_err();

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
                            }
                        };
                    }
                    ConnectionCommand::OpenSend { respond_to } => {
                        let send_res = self.connection.open_send_stream().await;

                        match send_res {
                            Ok(stream) => {
                                let quic_send =
                                    QuicSendStream::new(Handle::current(), stream, self.parent_id);
                                let send_err = respond_to.send(Ok(quic_send)).is_err();

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
                            }
                        }
                    }
                    ConnectionCommand::AcceptReceive { respond_to } => {
                        let accept_res = self.connection.accept_receive_stream().await;

                        match accept_res {
                            Ok(rec_opt) => {
                                let mapped = rec_opt.map(|rec_stream| {
                                    QuicReceiveStream::new(
                                        Handle::current(),
                                        rec_stream,
                                        self.parent_id,
                                    )
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
                            }
                        }
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
                    ConnectionCommand::Accept { respond_to } => todo!(),
                }
            }
        }

        self.disconnect_flag
            .unwrap_or(ConnectionDisconnectReason::InternalError(Arc::new(
                MissingErrorData,
            )))
    }
}

#[derive(Debug)]
pub enum StreamPollError {
    None,
    Error(TaskError),
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
