use std::{error::Error, net::SocketAddr, sync::Arc};

use bevy::{
    app::Plugin,
    ecs::system::ResMut,
    log::error,
    prelude::{Resource, World},
};
use bevy_transport::NetworkUpdate;
use s2n_quic::{client::Connect, Client};
use tokio::sync::mpsc::{self, error::TryRecvError, Receiver, Sender};

use crate::{common::ConnectionState, TokioRuntime};

const NEW_CONN_BATCH_SIZE: usize = 5;

#[derive(Resource)]
pub struct QuicClient {
    runtime: tokio::runtime::Handle,
    client: Arc<Client>,
    conn_receiver: Receiver<ConnectionState>,
    conn_sender: Arc<Sender<ConnectionState>>,
}

pub struct QuicClientConfig {
    max_connections: usize,
}

impl QuicClient {
    pub fn init_client(world: &mut World) -> Result<(), Box<dyn Error>> {
        // TODO: TLS certificate support
        let client = Client::builder().with_io("0.0.0.0:0")?.start()?;
        let (tx, rx) = mpsc::channel(1);

        let quic_client = Self {
            runtime: world
                .get_resource_or_init::<TokioRuntime>()
                .handle()
                .clone(),
            client: Arc::new(client),
            conn_receiver: rx,
            conn_sender: Arc::new(tx),
        };

        world.insert_resource(quic_client);
        Ok(())
    }

    pub fn try_connect(&mut self, addr: SocketAddr) {
        let connect = Connect::new(addr);
        let client = self.client.clone();
        let sender = self.conn_sender.clone();
        let runtime = self.runtime.clone();

        runtime.spawn(async move {
            let connection: ConnectionState = client.connect(connect).await.into();
            let _ = sender.send(connection).await;
        });
    }

    pub fn handle_connections(&mut self) -> [Option<ConnectionState>; NEW_CONN_BATCH_SIZE] {
        let rec = &mut self.conn_receiver;
        let mut flag = false;
        let data: [Option<ConnectionState>; NEW_CONN_BATCH_SIZE] = std::array::from_fn(|_| {
            // Avoids fragmentation of the returned data.
            // First none we encounter means we initialize rest of the data with None.
            if flag {
                return None;
            }

            let opt = rec.try_recv();

            match opt {
                Ok(state) => Some(state),
                Err(e) => {
                    match e {
                        TryRecvError::Empty => (), // Do nothing we don't care,
                        TryRecvError::Disconnected => error!("Connection sender pipe was broken. Was the sender or receiver dropped/closed before the client structure was dropped?"),
                    }
                    flag = true;
                    None
                }
            }
        });

        data
    }
}

pub struct QuicClientPlugin;

impl Plugin for QuicClientPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(NetworkUpdate, client_update);
    }
}

pub fn client_update(mut client: ResMut<QuicClient>) {
    let new_conns = client.handle_connections();
}
