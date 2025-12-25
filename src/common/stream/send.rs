use std::error::Error;

use bevy::log::tracing::Instrument;
use bevy::log::{error, info, warn};
use bytes::Bytes;
use s2n_quic::stream::SendStream;
use tokio::runtime::Handle;
use tokio::select;
use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinHandle;

use crate::common::HandleChannelError;

const DEBUG_CHANNEL_SIZE: usize = 256;
const CONTROL_CHANNEL_SIZE: usize = 256;
const OUTBOUND_CHANNEL_SIZE: usize = 1024;

pub struct QuicSendStream {
    send_task: JoinHandle<()>,
    outbound_data: Sender<Bytes>,
    outbound_control: Sender<SendControlMessage>,
    send_errors: Receiver<Box<dyn Error + Send + Sync>>,
    stream_id: u64,
}

impl QuicSendStream {
    pub fn new(runtime: Handle, send: SendStream) -> Self {
        let stream_id = send.id();

        let (send_error_sender, send_errors) = mpsc::channel(DEBUG_CHANNEL_SIZE);
        let (outbound_control, outbound_control_receiver) = mpsc::channel(CONTROL_CHANNEL_SIZE);
        let (outbound_data, outbound_data_receiver) = mpsc::channel(OUTBOUND_CHANNEL_SIZE);

        let span = bevy::log::info_span!("quic_send_task");
        let send_task = runtime.spawn(
            outbound_send_task(
                send,
                outbound_control_receiver,
                outbound_data_receiver,
                send_error_sender,
            )
            .instrument(span),
        );

        Self {
            send_task,
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
        !self.send_task.is_finished()
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
}

async fn outbound_send_task(
    mut send: SendStream,
    mut control: Receiver<SendControlMessage>,
    mut outbound_receiver: Receiver<Bytes>,
    send_errors: Sender<Box<dyn Error + Send + Sync>>,
) {
    let addr = send.connection().remote_addr();
    let id = send.id();
    info!("Opened send stream from: {:?}, with ID: {}", addr, id);

    let mut send_buf = Vec::with_capacity(OUTBOUND_CHANNEL_SIZE);

    'running: loop {
        let mut break_flag = false;

        select! {
            count = outbound_receiver.recv_many(&mut send_buf, OUTBOUND_CHANNEL_SIZE) => {
                // channel closed
                if count == 0 {
                    warn!(
                        "Outbound send channel is closed. Closing send stream from: {:?}, with ID: {}",
                        addr, id
                    );

                    break_flag = true;
                }

                let err_opt = send.send_vectored(&mut send_buf[..count]).await;
                send_buf.clear();

                if let Err(err) = err_opt {
                    match &err {
                        s2n_quic::stream::Error::InvalidStream { source, .. }
                        | s2n_quic::stream::Error::SendAfterFinish { source, .. } => {
                            error!(
                                "Send stream from: {:?}, ID: {}, is in an invalid state:\n{}",
                                addr, id, source
                            );
                        }
                        s2n_quic::stream::Error::StreamReset {
                            error, source: _, ..
                        } => {
                            error!(
                                "Send stream from: {:?}, ID: {}, has encountered a stream reset:\n{}",
                                addr, id, error
                            );
                            break_flag = true;
                        }

                        _ => {}
                    }

                    send_errors.try_send(Box::new(err)).handle_err();
                }
            }

            cmd_opt = control.recv() => {
                if let Some(cmd) = cmd_opt {
                    match cmd {
                        SendControlMessage::CloseAndQuit => {
                            let res = send.close().await;

                            if let Err(e) = res {
                                error!(
                                    "Send stream from: {:?}, ID: {}, errored when closing stream:\n{}",
                                    addr, id, e
                                );

                                send_errors.try_send(Box::new(e)).handle_err();
                            }
                        }
                        SendControlMessage::Flush => {
                            let res = send.flush().await;

                            if let Err(e) = res {
                                error!(
                                    "Send stream from: {:?}, ID: {}, errored when flushing stream:\n{}",
                                    addr, id, e
                                );

                                send_errors.try_send(Box::new(e)).handle_err();
                            }
                        }
                    }
                }
                else {
                    // Control channel has been dropped
                    info!(
                        "Control channel for send stream ID: {} has been dropped. Quitting...",
                        id
                    );
                    break_flag = true;
                };
            }
        }

        if break_flag {
            break 'running;
        }
    }

    info!(
        "Send stream from: {:?}, with ID: {}, has been closed",
        addr, id
    );

    let dropped_count = outbound_receiver.len();

    if dropped_count > 0 {
        warn!(
            "Send stream ID: {} dropped {} messages, this will result in loss of data being sent",
            id, dropped_count
        )
    }
}

enum SendControlMessage {
    CloseAndQuit,
    Flush,
}
