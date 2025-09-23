use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
    connection::Error as ConnectionError,
    provider::StartError,
};
use tokio::runtime::Handle;

use crate::common::connection::{
    QuicConnectionAttempt,
    id::{ConnectionId, ConnectionIdGenerator},
    runtime::TokioRuntime,
};

#[derive(Component)]
pub struct QuicClient {
    runtime: Handle,
    client: Client,
    id_gen: ConnectionIdGenerator,
}

// TODO: you can't actually use the builder from sync contexts :))))
// This is going to need to be reworked, as is the server code.
impl QuicClient {
    pub fn new(runtime: &TokioRuntime) -> Self {
        let client_handle = runtime.spawn(build());
        let client = runtime.block_on(client_handle).unwrap();

        QuicClient {
            runtime: runtime.handle().clone(),
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

async fn build() -> Client {
    Client::builder()
        .with_io("0.0.0.0:0")
        .expect("Unable to build client... are we... out of sockets??")
        .start()
        .expect("Unable to start client")
}
