use std::{error::Error, net::SocketAddr, sync::Arc};

use bevy::prelude::{Resource, World};
use s2n_quic::{client::Connect, Client};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{common::ConnectionState, TokioRuntime};

#[derive(Resource)]
pub struct QuicClient {
    runtime: tokio::runtime::Handle,
    connections: Vec<SocketAddr>,
    client: Arc<Client>,
    connection_rx: Receiver<ConnectionState>,
    connection_tx: Sender<ConnectionState>,
}

impl QuicClient {
    fn init_client(world: &mut World) -> Result<(), Box<dyn Error>> {
        // TODO: TLS certificate support
        let client = Client::builder().with_io("0.0.0.0:0")?.start()?;
        let (tx, rx) = mpsc::channel(1);

        let quic_client = Self {
            runtime: world
                .get_resource_or_init::<TokioRuntime>()
                .handle()
                .clone(),
            connections: Vec::new(),
            client: Arc::new(client),
            connection_rx: rx,
            connection_tx: tx,
        };

        world.insert_resource(quic_client);
        Ok(())
    }

    fn try_connect(&mut self, addr: SocketAddr) {
        let connect = Connect::new(addr);
        let client = self.client.clone();
        let runtime = self.runtime.clone();

        runtime.spawn(async move {
            let connection = client.connect(connect).await;
        });
    }
}
