use std::{error::Error, net::SocketAddr, sync::Arc};

use bevy::{
    app::Plugin,
    ecs::system::ResMut,
    log::{error, warn},
    prelude::{Resource, World},
};
use bevy_transport::{
    message::{InboundMessage, OutboundMessage},
    transport::NetReceiver,
    NetworkUpdate,
};
use s2n_quic::{client::Connect, Client, Connection};
use tokio::sync::mpsc::{self, error::TryRecvError, Receiver, Sender};

use crate::{common::ConnectionState, TokioRuntime};

const NEW_CONN_BATCH_SIZE: usize = 5;

#[derive(Resource)]
struct QuicClient {
    runtime: tokio::runtime::Handle,
    client: Arc<Client>,
    conn_receiver: Receiver<ConnectionState>,
    conn_sender: Arc<Sender<ConnectionState>>,
    connection: Option<Connection>,
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
            connection: None,
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

    pub fn rec_data(&mut self) -> InboundMessage {
        todo!()
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

pub fn client_update(mut client: ResMut<QuicClient>) {
    let mut new_conns = client.handle_connections();

    for conn_opt in new_conns.iter_mut() {
        if let Some(conn) = conn_opt.take() {
            match conn {
                ConnectionState::Connected(connection) => {
                    if let Some(client_conn) = &client.connection {
                        let addr_res = client_conn.remote_addr();

                        let addr_str: String = if let Ok(addr) = addr_res {
                            addr.to_string()
                        } else {
                            addr_res.err().unwrap().to_string()
                        };

                        warn!(
                            "bevy quic currently only supports one connection. disconnecting new connection: {}",
                            addr_str
                        );

                        // TODO: Create enum of error codes
                        let temp_code: u32 = 0;
                        connection.close(temp_code.into());

                        return;
                    }

                    client.connection = Some(connection);
                }
                ConnectionState::Failed(error) => warn!("connection failed: {}", error),
                ConnectionState::InProgress => todo!(),
            }
        } else {
            break;
        }
    }
}
