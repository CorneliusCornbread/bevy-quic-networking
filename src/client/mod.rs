use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
    connection::Error as ConnectionError,
};
use tokio::runtime::Handle;

use crate::common::connection::{
    QuicConnectionAttempt,
    id::{ConnectionId, ConnectionIdGenerator},
};

#[derive(Component)]
pub struct QuicClient {
    runtime: Handle,
    client: Client,
    id_gen: ConnectionIdGenerator,
}

impl QuicClient {
    pub fn new(runtime: Handle, client: Client) -> Self {
        QuicClient {
            runtime,
            client,
            id_gen: Default::default(),
        }
    }

    pub(crate) fn open_connection(
        &mut self,
        connect: Connect,
    ) -> (QuicConnectionAttempt, ConnectionId) {
        let client = &self.client;
        let attempt = client.connect(connect);

        let conn_task = self.runtime.spawn(create_connection(attempt));

        (
            QuicConnectionAttempt::new(self.runtime.clone(), conn_task),
            self.id_gen.generate_id(),
        )
    }
}

async fn create_connection(attempt: ConnectionAttempt) -> Result<Connection, ConnectionError> {
    attempt.await
}
