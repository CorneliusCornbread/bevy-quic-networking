use aeronet_io::{bytes::Bytes, packet::RecvPacket};
use bevy::log::{error, info, tracing::Instrument, warn};
use s2n_quic::application::Error as ErrorCode;
use s2n_quic::stream::ReceiveStream;
use std::error::Error;
use tokio::{
    runtime::Handle,
    select,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
    time::Instant as TokioInstant,
};

use crate::common::HandleChannelError;

const CHANNEL_BUFF_SIZE: usize = 256;

pub struct QuicReceiveStream {
    rec_task: JoinHandle<()>,
    inbound_data: Receiver<RecvPacket>,
    inbound_control: Sender<RecControlMessage>,
    receive_errors: Receiver<Box<dyn Error + Send + Sync>>,
    stream_id: u64,
}

impl QuicReceiveStream {
    pub fn new(runtime: Handle, rec: ReceiveStream) -> Self {
        let stream_id = rec.id();

        let (inbound_control, inbound_control_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (inbound_data_sender, inbound_data) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (receive_error_sender, receive_errors) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let span = bevy::log::info_span!("quic_rec_task");
        let rec_task = runtime.spawn(
            rec_task(
                rec,
                inbound_control_receiver,
                inbound_data_sender,
                receive_error_sender,
            )
            .instrument(span),
        );

        Self {
            rec_task,
            inbound_data,
            inbound_control,
            receive_errors,
            stream_id,
        }
    }

    pub fn poll_recv(&mut self) -> Option<RecvPacket> {
        if self.inbound_data.is_empty() {
            return None;
        }

        self.inbound_data.blocking_recv()
    }

    pub fn blocking_recv_many(&mut self, buffer: &mut Vec<RecvPacket>, limit: usize) -> usize {
        self.inbound_data.blocking_recv_many(buffer, limit)
    }

    pub fn is_open(&self) -> bool {
        !self.rec_task.is_finished()
    }

    pub fn stop_send(&mut self, err_code: ErrorCode) {
        let Err(_e) = self
            .inbound_control
            .blocking_send(RecControlMessage::StopSend(err_code))
        else {
            return;
        };

        info!(
            "Stop_send() called on closed connection with ID: {}.",
            self.stream_id
        );
    }

    pub fn print_rec_errors(&mut self) {
        while !self.receive_errors.is_empty() {
            let opt = self.receive_errors.blocking_recv();

            if let Some(err) = opt {
                error!("Receiver error: {}", err);
            }
        }
    }
}

const DEFAULT_SEND_ERR: ErrorCode = ErrorCode::UNKNOWN;

enum RecControlMessage {
    StopSend(ErrorCode),
}

impl Default for RecControlMessage {
    fn default() -> Self {
        Self::StopSend(DEFAULT_SEND_ERR)
    }
}

async fn rec_task(
    mut rec: ReceiveStream,
    mut control: Receiver<RecControlMessage>,
    inbound_sender: Sender<RecvPacket>,
    receive_errors: Sender<Box<dyn Error + Send + Sync>>,
) {
    const READ_TIMEOUT_MS: u64 = 50; // How long we wait for each receive read before moving on to command checks
    const BUFF_SIZE: usize = 100; // How big the receive buffer of Bytes chunks we can receive at once is
    const MAX_COMMAND_COUNT: usize = 10; // How many commands do we read each loop

    let addr = rec.connection().remote_addr();
    let id = rec.id();
    info!("Opened receive stream from: {:?}, with ID: {}", addr, id);

    let mut read_buf: [Bytes; BUFF_SIZE] = std::array::from_fn(|_| Bytes::new());

    // TODO: implement controls
    'running: loop {
        select! {
            biased;

            result = rec.receive_vectored(&mut read_buf) => {
                match result {
                    Ok((size, is_open)) => {
                        let instant = TokioInstant::now();

                        for data in &mut read_buf[0..size] {
                            let payload = std::mem::take(data);

                            let packet = RecvPacket {
                                recv_at: instant.into_std(),
                                payload,
                            };

                            inbound_sender.try_send(packet).handle_err();
                        }

                        if !is_open {
                            info!("Closing receive stream from: {:?}, with ID: {}", addr, id);
                            break 'running;
                        }
                    },
                    Err(e) => {
                        match e {
                            s2n_quic::stream::Error::ConnectionError { error , .. } => {
                                error!("Receive stream connection error: {error}");
                                receive_errors.try_send(Box::new(e)).handle_err();
                                break 'running;
                            }

                            s2n_quic::stream::Error::InvalidStream { source, .. } => {
                                error!("Invalid receive stream: {source}");
                                receive_errors.try_send(Box::new(e)).handle_err();
                                break 'running;
                            }

                            s2n_quic::stream::Error::StreamReset { error, source , .. } => {
                                warn!("Stream reset: {error}, Source: {source}");
                                receive_errors.try_send(Box::new(e)).handle_err();
                            }

                            _ => {
                                error!("Error when reading from receive stream: {}", e);
                                receive_errors.try_send(Box::new(e)).handle_err();
                            },
                        }
                    },
                }
            }

            _ = tokio::time::sleep(std::time::Duration::from_millis(READ_TIMEOUT_MS)) => {
                // Do nothing
            }
        }

        'cmd_loop: for _i in 0..MAX_COMMAND_COUNT {
            if control.is_empty() {
                continue 'running;
            }

            let Some(cmd) = control.recv().await else {
                warn!(
                    "Receive control panel is closed. Closing receive stream from: {:?}, with ID: {}",
                    addr, id
                );
                break 'running;
            };

            match cmd {
                RecControlMessage::StopSend(error_code) => {
                    let Err(stream_err) = rec.stop_sending(error_code) else {
                        continue 'cmd_loop;
                    };

                    warn!("Stream error on receive stop_send() exiting:\n{stream_err}");
                    break 'running;
                }
            }
        }
    }
}
