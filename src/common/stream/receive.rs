use aeronet::io::{bytes::Bytes, packet::RecvPacket};
use bevy::{
    ecs::component::Component,
    log::{error, info, info_once, tracing::Instrument, warn},
};
use s2n_quic::application::Error as ErrorCode;
use s2n_quic::stream::ReceiveStream;
use std::error::Error;
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
    time::Instant as TokioInstant,
};

use crate::common::HandleChannelError;

const CHANNEL_BUFF_SIZE: usize = 256;

#[derive(Component)]
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

        let span = bevy::log::info_span!("rec_task");
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

        info!("received data");
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
    //let mut command_buf = Vec::new();

    const BUFF_SIZE: usize = 100;
    let mut read_buf: [Bytes; BUFF_SIZE] = std::array::from_fn(|_| Bytes::new());

    'running: loop {
        info!("Receive!");
        let mut break_flag = false;

        /*         let command_count = control.recv_many(&mut command_buf, 100).await;

        for command in command_buf[..command_count].iter() {
            match command {
                RecControlMessage::StopSend(code) => {
                    match rec.stop_sending(*code) {
                        Ok(_) => info!("Send stream closed, exiting send stream task..."),
                        Err(e) => {
                            warn!(
                                "Send stream errored when closing: {e}\n Exiting send stream task..."
                            )
                        }
                    }
                    break_flag = true;
                }
            }
        } */

        match rec.receive().await {
            Ok(data) => {
                if let Some(packet) = data {
                    info!("Data received");
                    let instant = TokioInstant::now();

                    let packet = RecvPacket {
                        recv_at: instant.into_std(),
                        payload: packet.clone(),
                    };

                    inbound_sender.try_send(packet).handle_err();
                } else {
                    info!("Stream closed?");
                }
            }
            Err(e) => {
                receive_errors.try_send(Box::new(e)).handle_err();
                break_flag = true;
            }
        }

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

        if break_flag {
            break 'running;
        }
    }
}
