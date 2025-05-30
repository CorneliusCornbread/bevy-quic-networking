use std::error::Error;

use aeronet::io::{bytes::Bytes, packet::RecvPacket};
use bevy::{
    ecs::component::Component,
    log::{error, info, warn},
};
use s2n_quic::{
    application::Error as ErrorCode,
    stream::{BidirectionalStream, ReceiveStream, SendStream},
};
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, error::TrySendError, Receiver, Sender},
    task::JoinHandle,
    time::Instant as TokioInstant,
};

use crate::common::StreamId;

const CHANNEL_BUFF_SIZE: usize = 256;
const CONNECTION_CLOSED_ERR_CODE: u64 = 200;

pub type QuicSession = QuicSessionInternal;

enum SendControlMessage {
    CloseAndQuit,
    Flush,
}

enum RecControlMessage {
    StopSend(ErrorCode),
}

#[derive(Component)]
pub(crate) struct QuicSessionInternal {
    runtime: Handle,
    send_task: JoinHandle<()>,
    rec_task: JoinHandle<()>,
    outbound: Sender<Bytes>,
    inbound: Receiver<RecvPacket>,
    outbound_control: Sender<SendControlMessage>,
    inbound_control: Sender<RecControlMessage>,
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
                            warn!("Send stream errored when closing: {e}\n Exiting send stream task...")
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

async fn rec_task(
    mut rec: ReceiveStream,
    mut control: Receiver<RecControlMessage>,
    inbound_sender: Sender<RecvPacket>,
    receive_errors: Sender<Box<dyn Error + Send + Sync>>,
) {
    let mut command_buf = Vec::new();

    const BUFF_SIZE: usize = 100;
    let mut read_buf: [Bytes; BUFF_SIZE] = std::array::from_fn(|_| Bytes::new());

    'running: loop {
        let mut break_flag = false;

        let command_count = control.recv_many(&mut command_buf, 100).await;

        for command in command_buf[..command_count].iter() {
            match command {
                RecControlMessage::StopSend(code) => {
                    match rec.stop_sending(*code) {
                        Ok(_) => info!("Send stream closed, exiting send stream task..."),
                        Err(e) => {
                            warn!("Send stream errored when closing: {e}\n Exiting send stream task...")
                        }
                    }
                    break_flag = true;
                }
            }
        }

        let data_res = rec.receive_vectored(&mut read_buf).await;

        match data_res {
            Ok((size, is_open)) => {
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
        }

        if break_flag {
            break 'running;
        }
    }
}

trait HandleChannelError {
    fn handle_err(&self);
}

impl<T> HandleChannelError for Result<(), TrySendError<T>> {
    fn handle_err(&self) {
        if let Err(send_err) = self {
            error!("Error buffer for async task is full, the following error will be dropped: {send_err}");
        }
    }
}
