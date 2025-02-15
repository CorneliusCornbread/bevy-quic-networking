use std::{error::Error, net::SocketAddr, sync::Arc};

use aeronet::io::bytes::Bytes;
use bevy::{
    ecs::{component::Component, system::ResMut, world::FromWorld},
    log::{error, warn},
    prelude::{Resource, World},
};
use s2n_quic::{
    client::Connect,
    stream::{BidirectionalStream, SendStream},
    Client, Connection, Server,
};
use tokio::{
    sync::mpsc::{self, error::TryRecvError, Receiver, Sender},
    task::JoinHandle,
};

use crate::{common::TransportData, TokioRuntime};

const NEW_CONN_BATCH_SIZE: usize = 5;
const SERVER_ADDR: &str = "127.0.0.1:7777";

#[derive(Component)]
struct QuicServer {
    runtime: tokio::runtime::Handle,
    socket_rec_channel: Receiver<TransportData>,
    socket_send_channel: Sender<Bytes>,
    stream: Option<Arc<BidirectionalStream>>,
    send_task: JoinHandle<()>,
    rec_task: JoinHandle<()>,
}

impl QuicServer {
    pub fn start_server(world: &mut World) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder().with_io(SERVER_ADDR)?.start()?;
        let runtime = world
            .get_resource_or_init::<TokioRuntime>()
            .handle()
            .clone();

        let (outbount_sender, outbound_receiver) = mpsc::channel(32);
        let (inbound_sender, inbound_receiver) = mpsc::channel(32);
        let (new_conn_sender, new_conn_receiver) = mpsc::channel(32);

        let send_task = runtime.spawn(async move {
            Self::outbound_send_task(outbound_receiver, new_conn_receiver).await
        });

        let rec_task = runtime.spawn(async move {
            Self::inbound_rec_task(server, inbound_sender, new_conn_sender).await
        });

        let quic_server = Self {
            runtime,
            socket_rec_channel: inbound_receiver,
            socket_send_channel: outbount_sender,
            send_task,
            rec_task,
            stream: None,
        };

        Ok(quic_server)
    }

    async fn inbound_rec_task(
        mut server: Server,
        sender: Sender<TransportData>,
        new_conns: Sender<SendStream>,
    ) {
        let mut connections = Vec::new();
        'running: loop {
            while let Some(mut con) = server.accept().await {
                let _ = con.keep_alive(true);

                if let Ok(Some(stream)) = con.accept_bidirectional_stream().await {
                    let (rec, send) = stream.split();
                    connections.push(rec);
                    new_conns
                        .send(send)
                        .await
                        .expect("Error handling new connection");
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

#[derive(Resource)]
pub struct QuicClientConfig {
    max_connections: usize,
}

impl QuicClientConfig {
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }
}
