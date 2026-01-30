use bevy::log::tracing::Instrument;
use bevy::log::{error, info, warn};
use bytes::Bytes;
use s2n_quic::stream::SendStream;
use std::error::Error;
use tokio::runtime::Handle;
use tokio::select;
use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::common::HandleChannelError;
use crate::common::stream::disconnect::StreamDisconnectReason;
use crate::common::stream::task_state::StreamTaskState;

type AddrResult = Result<std::net::SocketAddr, s2n_quic::connection::Error>;

/// How many errors can be sent at a single time without being dropped
const DEBUG_CHANNEL_SIZE: usize = 32;
/// How many commands can be sent to the send socket without being processed before being dropped
const CONTROL_CHANNEL_SIZE: usize = 32;
/// How many messages can sit between async and bevy before being dropped
const OUTBOUND_CHANNEL_SIZE: usize = 128;

/// Minimum size of the send buffer of Bytes chunks we can receive at once is to send to bevy
const MIN_OUTBOUND_BUF_SIZE: usize = 64;
/// Maximum size of the send buffer of Bytes chunks we can receive at once is to send to bevy
const MAX_OUTBOUND_BUF_SIZE: usize = 128;

pub struct QuicSendStream {
    task_state: StreamTaskState,
    outbound_data: Sender<Bytes>,
    outbound_control: Sender<SendControlMessage>,
    send_errors: Receiver<Box<dyn Error + Send + Sync>>,
    stream_id: u64,
}

impl QuicSendStream {
    pub fn new(runtime: Handle, send: SendStream) -> Self {
        let stream_id = send.id();
        let addr = send.connection().local_addr();

        let (send_error_sender, send_errors) = mpsc::channel(DEBUG_CHANNEL_SIZE);
        let (outbound_control, outbound_control_receiver) = mpsc::channel(CONTROL_CHANNEL_SIZE);
        let (outbound_data, outbound_data_receiver) = mpsc::channel(OUTBOUND_CHANNEL_SIZE);

        let task = SendTask {
            send,
            control: outbound_control_receiver,
            outbound_receiver: outbound_data_receiver,
            send_errors: send_error_sender,
            disconnect_flag: None,
            addr,
            id: stream_id,
        };

        let span = bevy::log::info_span!("quic_send_task");
        let send_task = runtime.spawn(task.start().instrument(span));
        let task_state = StreamTaskState::new(runtime, send_task);

        Self {
            task_state,
            outbound_data,
            outbound_control,
            send_errors,
            stream_id,
        }
    }

    /// Returns `Some(())` in the event the close event was successful, if it wasn't
    /// it's due to the Receiver of the message being dropped. In which case
    /// it's likely the async task has been shut down, already quit, or crashed.
    pub fn close(&mut self) -> Option<()> {
        self.outbound_control
            .blocking_send(SendControlMessage::CloseAndQuit)
            .ok()
    }

    /// Returns `Some(())` in the event the flush event was successful, if it wasn't
    /// it's due to the Receiver of the message being dropped. In which case
    /// it's likely the async task has been shut down, already quit, or crashed.
    pub fn flush(&mut self) -> Option<()> {
        self.outbound_control
            .blocking_send(SendControlMessage::Flush)
            .ok()
    }

    /// Checks if the async task for the stream is still running, in which case
    /// the stream should still be open, if not the task should finish on its own.
    pub fn is_open(&self) -> bool {
        !self.task_state.is_finished()
    }

    pub fn send(&mut self, data: Bytes) -> Result<(), TrySendError<Bytes>> {
        self.outbound_data.try_send(data)
    }

    /// Take a vector of bytes and send bytes until an error is hit
    /// or until the vector is emptied.
    pub fn send_many_drain(&mut self, data: &mut Vec<Bytes>) -> Result<(), SendError<Bytes>> {
        let mut sent_count = 0;
        let mut res = Ok(());

        for item in data.iter() {
            res = self.outbound_data.blocking_send(item.clone());
            if res.is_err() {
                break;
            }

            sent_count += 1;
        }

        if sent_count == data.len() {
            data.clear();
        } else {
            data.drain(..sent_count);
        }

        res
    }

    pub fn log_outstanding_errors(&mut self) {
        while !self.send_errors.is_empty() {
            let Some(err) = self.send_errors.blocking_recv() else {
                continue;
            };

            error!("Sender ID: {}, encountered error:\n{}", self.stream_id, err);
        }
    }

    pub fn get_disconnect_reason(&mut self) -> Option<StreamDisconnectReason> {
        self.task_state.get_disconnect_reason()
    }
}

struct SendTask {
    pub(crate) send: SendStream,
    pub(crate) control: Receiver<SendControlMessage>,
    pub(crate) outbound_receiver: Receiver<Bytes>,
    pub(crate) send_errors: Sender<Box<dyn Error + Send + Sync>>,
    pub(crate) disconnect_flag: Option<StreamDisconnectReason>,
    pub(crate) addr: AddrResult,
    pub(crate) id: u64,
}

impl SendTask {
    async fn start(mut self) -> StreamDisconnectReason {
        info!(
            "Opened send stream from: {:?}, with ID: {}",
            self.addr, self.id
        );

        let mut send_buf = Vec::with_capacity(MIN_OUTBOUND_BUF_SIZE);

        'running: loop {
            select! {
                count = self.outbound_receiver.recv_many(&mut send_buf, MAX_OUTBOUND_BUF_SIZE) => {
                    // channel closed
                    if count == 0 {
                        warn!(
                            "Outbound send channel is closed. Closing send stream from: {:?}, with ID: {}",
                            self.addr, self.id
                        );

                        self.disconnect_flag = Some(StreamDisconnectReason::MspcChannelClosed{channel_name: "Outbound channel".into()})
                    }

                    let err_opt = self.send.send_vectored(&mut send_buf[..count]).await;
                    send_buf.clear();

                    if let Err(err) = err_opt {
                        match err {
                            s2n_quic::stream::Error::InvalidStream { source, .. }
                            | s2n_quic::stream::Error::SendAfterFinish { source, .. } => {
                                error!(
                                    "Send stream from: {:?}, ID: {}, is in an invalid state:\n{}",
                                    self.addr, self.id, source
                                );
                                self.disconnect_flag = Some(StreamDisconnectReason::InvalidStream)
                            }

                            s2n_quic::stream::Error::StreamReset {
                                error, source: _, ..
                            } => {
                                error!(
                                    "Send stream from: {:?}, ID: {}, has encountered a stream reset:\n{}",
                                    self.addr, self.id, error
                                );
                                self.disconnect_flag = Some(StreamDisconnectReason::Reset(error));
                            }

                            _ => {
                                error!(
                                    "Send stream from: {:?}, ID: {}, error:\n{}",
                                    self.addr, self.id, err
                                );
                            }
                        }

                        self.send_errors.try_send(Box::new(err)).handle_err();
                    }
                }

                cmd_opt = self.control.recv() => {
                    if let Some(cmd) = cmd_opt {
                        match cmd {
                            SendControlMessage::CloseAndQuit => {
                                let res = self.send.close().await;

                                if let Err(e) = res {
                                    error!(
                                        "Send stream from: {:?}, ID: {}, errored when closing stream:\n{}",
                                        self.addr, self.id, e
                                    );

                                    self.send_errors.try_send(Box::new(e)).handle_err();
                                }

                                self.disconnect_flag = Some(StreamDisconnectReason::UserClosed);
                            }

                            SendControlMessage::Flush => {
                                let res = self.send.flush().await;

                                if let Err(e) = res {
                                    error!(
                                        "Send stream from: {:?}, ID: {}, errored when flushing stream:\n{}",
                                        self.addr, self.id, e
                                    );

                                    self.send_errors.try_send(Box::new(e)).handle_err();
                                }
                            }
                        }
                    }
                    else {
                        // Control channel has been dropped
                        info!(
                            "Control channel for send stream ID: {} has been dropped. Quitting...",
                            self.id
                        );
                        self.disconnect_flag = Some(StreamDisconnectReason::MspcChannelClosed{channel_name: "Control channel".into()})
                    };
                }
            }

            if self.disconnect_flag.is_some() {
                break 'running;
            }
        }

        info!(
            "Send stream from: {:?}, with ID: {}, has been closed",
            self.addr, self.id
        );

        let dropped_count = self.outbound_receiver.len();

        if dropped_count > 0 {
            warn!(
                "Send stream ID: {} dropped {} messages, this will result in loss of data being sent",
                self.id, dropped_count
            )
        }

        if let Some(reason) = self.disconnect_flag {
            reason
        }
        // In theory this should never happen
        else {
            StreamDisconnectReason::NoReason
        }
    }
}

enum SendControlMessage {
    CloseAndQuit,
    Flush,
}
