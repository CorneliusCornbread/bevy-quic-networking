use bevy::{
    log::{tracing::Instrument, warn},
    prelude::{Deref, DerefMut},
};
use s2n_quic::{
    Connection,
    connection::Error as ConnectionError,
    stream::{BidirectionalStream, PeerStream, ReceiveStream},
};
use std::{error::Error, sync::Arc, time::Duration};
use tokio::{
    runtime::Handle,
    sync::{Mutex, mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

use crate::common::{
    attempt::{QuicActionAttempt, TaskError},
    connection::{
        disconnect::ConnectionDisconnectReason, id::ConnectionId, task_state::ConnectionTaskState,
    },
    stream::{
        QuicBidirectionalStreamAttempt, QuicReceiveStreamAttempt,
        id::{StreamId, StreamIdGenerator},
        receive::QuicReceiveStream,
        send::QuicSendStream,
    },
};

pub mod disconnect;
pub mod id;
pub mod plugin;
pub mod runtime;
pub mod task_state;

const CONNECTION_CHANNEL_SIZE: usize = 1024;
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
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

pub(crate) struct BidirectionalSessionAttempt(pub QuicBidirectionalStreamAttempt, pub StreamId);
pub(crate) struct ReceiveSessionAttempt(pub QuicReceiveStreamAttempt, pub StreamId);

#[derive(Debug)]
pub struct QuicConnection {
    runtime: Handle,
    task_state: ConnectionTaskState,
    conn_command_channel: mpsc::Sender<ConnectionCommand>,
    //connection_id: ConnectionId,
    id_gen: StreamIdGenerator,
}

impl QuicConnection {
    pub(crate) fn new(
        runtime: Handle,
        mut connection: Connection,
        //connection_id: ConnectionId,
    ) -> Self {
        let res = connection.keep_alive(true);

        let (send, rec) = mpsc::channel(CONNECTION_CHANNEL_SIZE);

        if let Err(e) = res {
            warn!(
                "Unable to mark new connection with keep alive, is the connection already closed? Reason: \"{}\"",
                e
            );
        }

        // TODO: Rework ID system, having IDs be in separate components is a real PITA.
        let span = bevy::log::info_span!("quic_connection_task", connection_id = connection.id());

        let task = ConnectionTask {
            connection,
            cmd_receiver: rec,
            disconnect_flag: None,
        };

        let handle = runtime.spawn(task.start().instrument(span));

        Self {
            runtime: runtime.clone(),
            task_state: ConnectionTaskState::new(runtime, handle),
            conn_command_channel: send,
            id_gen: Default::default(),
            //connection_id,
        }
    }

    pub(crate) fn accept_streams(&mut self) -> Result<(PeerStream, StreamId), StreamPollError> {
        todo!()

        /* let waker = Arc::new(futures::task::noop_waker_ref());
        let mut cx = std::task::Context::from_waker(&waker);

        let mut lock = self.connection.try_lock()?;
        let poll = lock.poll_accept(&mut cx);

        if let std::task::Poll::Ready(res) = poll
            && let Ok(opt) = res
            && let Some(stream) = opt
        {
            return Ok((stream, self.id_gen.generate_id()));
        }

        Err(StreamPollError::None) */
    }

    pub(crate) fn open_bidrectional_stream(&mut self) -> BidirectionalSessionAttempt {
        let (send, rec) = oneshot::channel();

        let cmd = ConnectionCommand::OpenBidirectional { respond_to: send };

        let attempt = BidirectionalSessionAttempt(
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), rec),
            self.generate_stream_id(),
        );

        // Just ignore channel errors, they'll get handled in the attempt regardless
        let _send_res = self.conn_command_channel.blocking_send(cmd);

        attempt
    }

    pub(crate) fn accept_receive_stream(&mut self) -> ReceiveSessionAttempt {
        todo!()

        /* let conn_task = self
            .runtime
            .spawn(accept_receive_task(self.connection.clone()));

        ReceiveSessionAttempt(
            QuicReceiveStreamAttempt::new(self.runtime.clone(), conn_task),
            self.generate_stream_id(),
        ) */
    }

    pub(crate) fn generate_stream_id(&mut self) -> StreamId {
        self.id_gen.generate_id()
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
    disconnect_flag: Option<ConnectionDisconnectReason>,
}

impl ConnectionTask {
    pub(crate) async fn start(mut self) -> ConnectionDisconnectReason {
        let mut cmd_buf = Vec::with_capacity(CONNECTION_CMD_BUFF_SIZE_MIN);

        'connected: loop {
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

                                let quic_send = QuicSendStream::new(Handle::current(), send_stream);
                                let quic_rec =
                                    QuicReceiveStream::new(Handle::current(), rec_stream);

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
                                let quic_send = QuicSendStream::new(Handle::current(), stream);
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
                                    QuicReceiveStream::new(Handle::current(), rec_stream)
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

                        break 'connected;
                    }
                }
            }
        }

        ConnectionDisconnectReason::UserClosed
    }
}

async fn open_bidirectional_task(
    mut conn: Connection,
) -> Result<(QuicReceiveStream, QuicSendStream), ConnectionError> {
    let bi_stream_res: Result<BidirectionalStream, ConnectionError>;

    bi_stream_res = conn.open_bidirectional_stream().await;

    let bi_stream = bi_stream_res?;
    let (rec, send) = bi_stream.split();

    let send_stream = QuicSendStream::new(Handle::current(), send);
    let rec_stream = QuicReceiveStream::new(Handle::current(), rec);

    Ok((rec_stream, send_stream))
}

async fn accept_receive_task(
    conn: Arc<Mutex<Connection>>,
) -> Result<Option<QuicReceiveStream>, ConnectionError> {
    let rec_stream_res: Result<Option<ReceiveStream>, ConnectionError>;

    {
        let mut conn = conn.lock().await;

        rec_stream_res = match timeout(Duration::from_millis(1), conn.accept_receive_stream()).await
        {
            Ok(res) => res,
            Err(_e) => Ok(None),
        }
    }

    if let Err(e) = rec_stream_res {
        return Err(e);
    }

    let opt = rec_stream_res.unwrap();

    if let None = opt {
        return Ok(None);
    }

    let stream = opt.unwrap();
    let quic_rec_stream = QuicReceiveStream::new(Handle::current(), stream);

    Ok(Some(quic_rec_stream))
}

#[derive(Debug)]
pub enum StreamPollError {
    None,
    Error(Box<dyn Error>),
}

impl<E> From<E> for StreamPollError
where
    E: Error + 'static,
{
    fn from(err: E) -> Self {
        StreamPollError::Error(Box::new(err))
    }
}
