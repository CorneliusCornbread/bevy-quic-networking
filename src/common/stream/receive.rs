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
}

impl QuicReceiveStream {
    pub fn new(runtime: Handle, rec: ReceiveStream) -> Self {
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

    pub fn print_rec_errors(&mut self) {
        while !self.receive_errors.is_empty() {
            let opt = self.receive_errors.blocking_recv();

            if let Some(err) = opt {
                error!("Receiver error: {}", err);
            }
        }
    }
}

enum RecControlMessage {
    StopSend(ErrorCode),
}

async fn rec_task(
    mut rec: ReceiveStream,
    mut control: Receiver<RecControlMessage>,
    inbound_sender: Sender<RecvPacket>,
    receive_errors: Sender<Box<dyn Error + Send + Sync>>,
) {
    let addr = rec.connection().remote_addr();
    let id = rec.id();
    info!("Opened receive stream from: {:?}, with ID: {}", addr, id);

    const BUFF_SIZE: usize = 100;
    let mut read_buf: [Bytes; BUFF_SIZE] = std::array::from_fn(|_| Bytes::new());

    const READ_TIMEOUT_MS: u64 = 100;

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
                            info!("Closing send stream from: {:?}, with ID: {}", addr, id);
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

        /* let poll_data = poll_fn(|cx| rec.poll_receive_vectored(&mut read_buf, cx)).await;

        match poll_data {
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
                    info!("Closing send stream from: {:?}, with ID: {}", addr, id);
                    break 'running;
                }
            }
            Err(e) => {
                error!("Error when reading from receive stream: {}", e);
                receive_errors.try_send(Box::new(e)).handle_err();
                break 'running;
            }
        } */

        /* let data_res = rec.receive_vectored(&mut read_buf).await;
        info!("rec vectored returned");

        match data_res {
            Ok((size, is_open)) => {
                info!("Data res: {}", size);

                let instant = TokioInstant::now();
                for data in read_buf[0..size].iter() {
                    let packet = RecvPacket {
                        recv_at: instant.into_std(),
                        payload: data.clone(),
                    };

                    inbound_sender.try_send(packet).handle_err();
                }

                if !is_open {
                    let id = rec.id();

                    info!("Receive stream id {id} was closed, quitting receive task...");
                    break_flag = true;
                }
            }
            Err(rec_err) => {
                receive_errors.try_send(Box::new(rec_err)).handle_err();
            }
        } */
    }
}
