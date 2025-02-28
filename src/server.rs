use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use aeronet::io::bytes::Bytes;
use ahash::AHasher;
use bevy::{
    ecs::component::Component,
    prelude::{Resource, World},
};
use s2n_quic::{
    stream::{BidirectionalStream, SendStream},
    Server,
};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::{
    common::{AddrHash, ConnectionId, TransportData},
    TokioRuntime,
};

const NEW_CONN_BATCH_SIZE: usize = 5;
const SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

#[derive(Component)]
pub struct QuicServer {
    runtime: tokio::runtime::Handle,
    socket_rec_channel: Receiver<TransportData>,
    socket_send_channel: Sender<Bytes>,
    connection_rec_channel: Receiver<ConnectionId>,
    stream: Option<Arc<BidirectionalStream>>,
    send_task: JoinHandle<()>,
    rec_task: JoinHandle<()>,
    hasher: AHasher,
    max_connections: usize,
}

impl QuicServer {
    pub fn start_server(
        world: &mut World,
        max_connections: usize,
        port: u16,
    ) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder().with_io(server_addr(port))?.start()?;
        let runtime = world
            .get_resource_or_init::<TokioRuntime>()
            .handle()
            .clone();

        let (outbount_sender, outbound_receiver) = mpsc::channel(32);
        let (inbound_sender, inbound_receiver) = mpsc::channel(32);
        let (new_conn_sender, new_conn_receiver) = mpsc::channel(32);
        let (bevy_new_conn_sender, bevy_new_conn_receiver) = mpsc::channel(32);

        let send_task = runtime.spawn(async move {
            Self::outbound_send_task(outbound_receiver, new_conn_receiver).await
        });

        let rec_task = runtime.spawn(async move {
            Self::inbound_rec_task(
                server,
                inbound_sender,
                new_conn_sender,
                bevy_new_conn_sender,
            )
            .await
        });

        let quic_server = Self {
            runtime,
            socket_rec_channel: inbound_receiver,
            socket_send_channel: outbount_sender,
            connection_rec_channel: bevy_new_conn_receiver,
            send_task,
            rec_task,
            stream: None,
            hasher: AHasher::default(),
            max_connections,
        };

        Ok(quic_server)
    }

    async fn inbound_rec_task(
        mut server: Server,
        sender: Sender<TransportData>,
        new_conns: Sender<SendStream>,
        bevy_new_conns: Sender<ConnectionId>,
    ) {
        let mut connections = Vec::new();

        'running: loop {
            while let Some(mut con) = server.accept().await {
                let _ = con.keep_alive(true);

                if let Ok(Some(stream)) = con.accept_bidirectional_stream().await {
                    let (rec, send) = stream.split();
                    connections.push(rec);

                    // TODO: change the sender to have a custom StreamId type
                    bevy_new_conns
                        .send(send.id().into())
                        .await
                        .expect("Error sending new connection to Bevy thread");

                    new_conns
                        .send(send)
                        .await
                        .expect("Error handling new connection");

                    let addr = con.id();

                    let data = TransportData::Connected(addr.into());
                    sender.send(data);
                } else {
                    // TODO: error codes
                    con.close(99_u32.into());
                }
            }

            for con in connections.iter_mut() {
                if let Ok(Some(data)) = con.receive().await {
                    let transport = TransportData::ReceivedData(data);
                    sender
                        .send(transport)
                        .await
                        .expect("Error sending received data");
                }
            }
        }
    }

    async fn outbound_send_task(
        mut receiver: Receiver<Bytes>,
        mut new_conns: Receiver<SendStream>,
    ) {
        let mut connections = Vec::new();

        'running: loop {
            while let Some(conn) = new_conns.recv().await {
                connections.push(conn);
            }

            while let Some(message) = receiver.recv().await {
                for conn in connections.iter_mut() {
                    // TODO: error handling/logging
                    let _err = conn.send(message.clone());
                }
            }
        }
    }
}

fn server_addr(port: u16) -> SocketAddr {
    SocketAddr::new(SERVER_IP, port)
}
