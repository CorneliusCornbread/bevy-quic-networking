use std::error::Error;

use aeronet::io::bytes::Bytes;
use bevy::ecs::component::Component;
use bevy::log::{info, warn};
use s2n_quic::stream::SendStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinHandle;

use crate::common::HandleChannelError;

const CHANNEL_BUFF_SIZE: usize = 256;

#[derive(Component)]
pub struct QuicSendStream {
    send_task: JoinHandle<()>,
    outbound_data: Sender<Bytes>,
    outbound_control: Sender<SendControlMessage>,
    send_errors: Receiver<Box<dyn Error + Send + Sync>>,
}

impl QuicSendStream {
    pub fn new(runtime: Handle, send: SendStream) -> Self {
        let (send_error_sender, send_errors) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (outbound_control, outbound_control_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (outbound_data, outbound_data_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let send_task = runtime.spawn(outbound_send_task(
            send,
            outbound_control_receiver,
            outbound_data_receiver,
            send_error_sender,
        ));

        Self {
            send_task,
            outbound_data,
            outbound_control,
            send_errors,
        }
    }
}

async fn outbound_send_task(
    mut send: SendStream,
    mut control: Receiver<SendControlMessage>,
    mut outbound_receiver: Receiver<Bytes>,
    send_errors: Sender<Box<dyn Error + Send + Sync>>,
) {
    let mut command_buf = Vec::new();

    'running: loop {
        let mut break_flag = false;

        let command_count = control.recv_many(&mut command_buf, 100).await;

        for i in 0..command_count {
            let command_opt = command_buf.get(i);

            if command_opt.is_none() {
                // in theory this should never happen
                break;
            }

            match command_opt.unwrap() {
                SendControlMessage::CloseAndQuit => {
                    match send.close().await {
                        Ok(_) => info!("Send stream closed, exiting send stream task..."),
                        Err(e) => {
                            warn!(
                                "Send stream errored when closing: {e}\n Exiting send stream task..."
                            )
                        }
                    }
                    break_flag = true;
                }
                SendControlMessage::Flush => {
                    let res = send.flush().await;
                    if let Err(e) = res {
                        send_errors.try_send(Box::new(e)).handle_err();
                    }
                }
            }
        }

        // If we're quitting send all the ready to send messages we have in the buffer.
        // If there are more they get dropped.
        if break_flag {
            let message_count = outbound_receiver.len();
            for _i in 0..message_count {
                send_data(&mut send, &mut outbound_receiver, &send_errors).await;
            }

            break 'running;
        }

        send_data(&mut send, &mut outbound_receiver, &send_errors).await;
    }
}

async fn send_data(
    send: &mut SendStream,
    outbound_receiver: &mut Receiver<Bytes>,
    send_errors: &Sender<Box<dyn Error + Send + Sync + 'static>>,
) {
    if let Some(data) = outbound_receiver.recv().await {
        let err_opt = send.send(data).await.err();

        if let Some(err) = err_opt {
            send_errors.try_send(Box::new(err)).handle_err();
        }
    }
}

enum SendControlMessage {
    CloseAndQuit,
    Flush,
}
