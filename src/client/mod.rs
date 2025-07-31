use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
    connection::Error as ConnectionError,
};
use tokio::runtime::Handle;

use crate::common::connection::attempt::QuicConnectionAttempt;

#[derive(Component)]
pub struct QuicClient {
    runtime: Handle,
    client: s2n_quic::Client,
}

impl QuicClient {
    pub fn new(runtime: Handle, client: Client) -> Self {
        QuicClient { runtime, client }
    }

    pub fn connect(&mut self, connect: Connect) -> QuicConnectionAttempt {
        let client = &self.client;
        let attempt = client.connect(connect);

        let conn_task = self.runtime.spawn(create_connection(attempt));

        QuicConnectionAttempt::new(self.runtime.clone(), conn_task)
    }
}

async fn create_connection(attempt: ConnectionAttempt) -> Result<Connection, ConnectionError> {
    attempt.await
}
