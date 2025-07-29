use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, atomic::AtomicBool},
};

use aeronet::io::SessionEndpoint;
use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
};
use tokio::{runtime::Handle, sync::RwLock, task::JoinHandle};

#[derive(Component)]
#[require(SessionEndpoint)]
pub struct QuicClientEndpoint {
    conn_task: JoinHandle<()>,
    conn_status: Arc<RwLock<ClientEndpointStatus>>,
}

impl QuicClientEndpoint {
    pub fn connect(
        runtime: Handle,
        client: &Client,
        connect: Connect,
    ) -> Result<Self, Box<dyn Error>> {
        let conn = client.connect(connect);
        let conn_status = Arc::new(RwLock::new(ClientEndpointStatus::Connecting));

        let conn_task = runtime.spawn(handle_connection(conn, conn_status.clone()));

        let endpoint = Self {
            conn_task,
            conn_status,
        };

        Ok(endpoint)
    }
}

async fn handle_connection(
    attempt: ConnectionAttempt,
    conn_status: Arc<RwLock<ClientEndpointStatus>>,
) {
    let conn_res = attempt.await;

    if let Err(e) = conn_res {
        let mut w_lock = conn_status.write().await;
        *w_lock = ClientEndpointStatus::Failed(e);
        return;
    }

    let conn = conn_res.unwrap();

    'running: loop {}
}

enum ClientEndpointStatus {
    Closed,
    Connecting,
    Connected,
    Failed(s2n_quic::connection::Error),
}
