use std::error::Error;

use aeronet::io::bytes::Bytes;
use bevy::{
    ecs::component::Component,
    log::{error, info, warn},
};
use s2n_quic::stream::{BidirectionalStream, ReceiveStream, SendStream, SplittableStream};
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, error::TrySendError, Receiver, Sender},
    task::JoinHandle,
    time::Instant,
};

use crate::common::StreamId;

const CHANNEL_BUFF_SIZE: usize = 256;

pub type QuicSession = QuicSessionInternal;

struct Packet(Bytes, Instant);

enum ControlMessage {
    CloseAndQuit,
    Flush,
}

#[derive(Component)]
pub(crate) struct QuicSessionInternal {
    runtime: Handle,
    send_task: JoinHandle<()>,
    rec_task: JoinHandle<()>,
    outbound: Sender<Packet>,
    inbound: Receiver<Packet>,
    outbound_control: Sender<ControlMessage>,
    inbound_control: Sender<ControlMessage>,
    send_errors: Receiver<Box<dyn Error + Send + Sync>>,
    receive_errors: Receiver<Box<dyn Error + Send + Sync>>,
    pub(crate) id: StreamId,
}

impl QuicSessionInternal {
    pub(crate) fn new(runtime: Handle, id: StreamId, stream: BidirectionalStream) -> Self {
        let (rec, send): (ReceiveStream, SendStream) = stream.split();

        let (outbound_control, outbound_control_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (inbound_control, inbound_control_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let (outbound_data, outbound_data_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (inbound_data_sender, inbound_data) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let (send_error_sender, send_errors) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (receive_error_sender, receive_errors) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let send_task = runtime.spawn(outbound_send_task(
            send,
            outbound_control_receiver,
            outbound_data_receiver,
            send_error_sender,
        ));
        let rec_task = runtime.spawn(rec_task(
            rec,
            inbound_control_receiver,
            inbound_data_sender,
            receive_error_sender,
        ));

        Self {
            runtime,
            send_task,
            rec_task,
            outbound: outbound_data,
            inbound: inbound_data,
            outbound_control,
            inbound_control,
            send_errors,
            receive_errors,
            id,
        }
    }
}

async fn outbound_send_task(
    mut send: SendStream,
    mut control: Receiver<ControlMessage>,
    mut outbound_receiver: Receiver<Packet>,
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
                ControlMessage::CloseAndQuit => {
                    match send.close().await {
                        Ok(_) => info!("Send stream closed, exiting send stream task..."),
                        Err(e) => {
                            warn!("Send stream errored when closing: {e}\n Exiting send stream task...")
                        }
                    }
                    break_flag = true;
                }
                ControlMessage::Flush => {
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
    outbound_receiver: &mut Receiver<Packet>,
    send_errors: &Sender<Box<dyn Error + Send + Sync + 'static>>,
) {
    if let Some(data) = outbound_receiver.recv().await {
        let err_opt = send.send(data.0).await.err();

        if let Some(err) = err_opt {
            send_errors.try_send(Box::new(err)).handle_err();
        }
    }
}

async fn rec_task(
    rec: ReceiveStream,
    control: Receiver<ControlMessage>,
    inbound_sender: Sender<Packet>,
    mut receive_errors: Sender<Box<dyn Error + Send + Sync>>,
) {
}

trait HandleChannelError {
    fn handle_err(&self);
}

impl<T> HandleChannelError for Result<(), TrySendError<T>> {
    fn handle_err(&self) {
        if let Err(send_err) = self {
            error!("Error buffer for send task is full, the following error will be dropped: {send_err}");
        }
    }
}
