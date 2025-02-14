use std::{error::Error, net::SocketAddr, sync::Arc};

use bevy::{
    ecs::system::ResMut,
    log::{error, warn},
    prelude::{Resource, World},
};
use s2n_quic::{
    client::Connect,
    stream::{BidirectionalStream, SendStream},
    Client, Connection, Server,
};
use tokio::sync::mpsc::{self, error::TryRecvError, Receiver, Sender};

use crate::{common::TransportData, TokioRuntime};

const NEW_CONN_BATCH_SIZE: usize = 5;

#[derive(Resource)]
struct QuicServer {
    runtime: tokio::runtime::Handle,
    socket_rec_channel: Receiver<TransportData>,
    socket_send_channel: Sender<Vec<u8>>,
    stream: Option<Arc<BidirectionalStream>>,
}

impl QuicServer {
    async fn inbound_rec_task(
        mut server: Server,
        mut sender: Sender<TransportData>,
        mut new_conns: Sender<SendStream>,
    ) {
        let mut connections = Vec::new();
        'running: loop {
            while let Some(mut con) = server.accept().await {
                if let Ok(Some(stream)) = con.accept_bidirectional_stream().await {
                    let (rec, send) = stream.split();
                    connections.push(rec);
                    new_conns
                        .send(send)
                        .await
                        .expect("Error handling new connection");

                    //connections.push(stream);
                } else {
                    // TODO: error codes
                    con.close(99_u32.into());
                }
            }

            for con in connections.iter_mut() {
                if let Ok(Some(data)) = con.receive().await {}
            }
        }
    }

    async fn outbound_send_task(
        mut Receiver: Receiver<Vec<u8>>,
        mut new_conns: Receiver<SendStream>,
    ) {
    }

    fn start_server(&self, world: &mut World) -> Result<Receiver<TransportData>, Box<dyn Error>> {
        let mut server = Server::builder().with_io("0.0.0.0:0")?.start()?;
        let runtime = self.runtime.clone();

        let (outbount_sender, outbound_receiver) = mpsc::channel(32);
        let (inbound_sender, inbound_receiver) = mpsc::channel(32);
        let (new_conn_sender, new_conn_receiver) = mpsc::channel(32);

        let quic_server = Self {
            runtime: world
                .get_resource_or_init::<TokioRuntime>()
                .handle()
                .clone(),
            socket_rec_channel: inbound_receiver,
            socket_send_channel: outbount_sender,
            stream: None,
        };

        quic_server.runtime.spawn(async move {
            Self::outbound_send_task(outbound_receiver, new_conn_receiver).await
        });

        quic_server.runtime.spawn(async move {
            Self::inbound_rec_task(server, inbound_sender, new_conn_sender).await
        });

        todo!();
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
