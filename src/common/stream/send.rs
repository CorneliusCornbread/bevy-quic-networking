use std::collections::btree_map::Range;
use std::error::Error;
use std::sync::Arc;
use std::task::Poll;

use aeronet::io::bytes::Bytes;
use bevy::ecs::component::Component;
use bevy::log::tracing::Instrument;
use bevy::log::{error, info, warn};
use s2n_quic::stream::SendStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc::error::{SendError, TrySendError};
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
        let waker = Arc::new(futures::task::noop_waker_ref());
        let mut cx = std::task::Context::from_waker(&waker);
        let mut buf = vec![];

        let poll = self.send_errors.poll_recv_many(&mut cx, &mut buf, 100);

        if let Poll::Ready(count) = poll {
            for err in &mut buf[..count] {
                error!("Send task error: {}", err);
            }
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

    //let mut command_buf = Vec::new();

    'running: loop {
        let mut break_flag = false;

        /* let command_count = control.recv_many(&mut command_buf, 100).await;

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
        */
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
