use aeronet::io::bytes::Bytes;
use bevy::ecs::component::Component;
use s2n_quic::stream::{BidirectionalStream, ReceiveStream, SendStream};
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
    time::Instant,
};

use crate::common::StreamId;

const CHANNEL_BUFF_SIZE: usize = 64;

pub type QuicSession = QuicSessionInternal;

struct Packet(Bytes, Instant);

enum ControlMessage {
    Disconnect,
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
    pub(crate) id: StreamId,
}

impl QuicSessionInternal {
    pub(crate) fn new(runtime: Handle, id: StreamId, stream: BidirectionalStream) -> Self {
        let (rec, send): (ReceiveStream, SendStream) = stream.split();

        let (outbound_control, outbound_control_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (inbound_control, inbound_control_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let (outbound_data, outbound_data_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (inbound_data_sender, inbound_data) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let send_task = runtime.spawn(outbound_send_task(
            send,
            outbound_control_receiver,
            outbound_data_receiver,
        ));
        let rec_task = runtime.spawn(rec_task(rec, inbound_control_receiver, inbound_data_sender));

        Self {
            runtime,
            send_task,
            rec_task,
            outbound: outbound_data,
            inbound: inbound_data,
            outbound_control,
            inbound_control,
            id,
        }
    }
}

async fn outbound_send_task(
    mut send: SendStream,
    control: Receiver<ControlMessage>,
    mut outbound_receiver: Receiver<Packet>,
) /*-> Result<(), s2n_quic::stream::Error>*/
{
    if let Some(data) = outbound_receiver.recv().await {
        let res: Result<(), s2n_quic::stream::Error> = send.send(data.0).await;
        //return res;
    }

    //Ok(())
}

async fn rec_task(
    rec: ReceiveStream,
    control: Receiver<ControlMessage>,
    inbound_sender: Sender<Packet>,
) {
}
