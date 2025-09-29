use std::{error::Error, sync::Arc};

use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
    connection::Error as ConnectionError,
    provider::StartError,
};
use tokio::{runtime::Handle, sync::Mutex};

use crate::common::connection::{
    QuicConnection, QuicConnectionAttempt,
    id::{ConnectionId, ConnectionIdGenerator},
    runtime::TokioRuntime,
};

#[derive(Component)]
pub struct QuicClient {
    runtime: Handle,
    client: Arc<Mutex<Client>>,
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
            client: Arc::new(Mutex::new(client)),
            id_gen: Default::default(),
        }
    }

    // TODO: REMOVE ME
    pub fn test_connection(&mut self, connect: Connect) -> Result<Connection, ConnectionError> {
        let conn: Result<Connection, ConnectionError> = self
            .runtime
            .block_on(client_connection(self.client.clone(), connect));

        conn
    }

    pub(crate) fn open_connection(
        &mut self,
        connect: Connect,
    ) -> (QuicConnectionAttempt, ConnectionId) {
        let client = &self.client.blocking_lock();
        let attempt = client.connect(connect);

        let conn_task = self.runtime.spawn(create_connection(attempt));

        (
            QuicConnectionAttempt::new(self.runtime.clone(), conn_task),
            self.id_gen.generate_id(),
        )
    }
}

async fn client_connection(
    client: Arc<Mutex<Client>>,
    connect: Connect,
) -> Result<Connection, ConnectionError> {
    let lock = client.lock().await;
    let attempt = lock.connect(connect);
    drop(lock);
    attempt.await
}

async fn create_connection(attempt: ConnectionAttempt) -> Result<Connection, ConnectionError> {
    attempt.await
}

async fn build() -> Client {
    let tls = s2n_quic_tls::Client::builder(); // TODO: implement TLS providers

    Client::builder()
        .with_io("0.0.0.0:0")
        .expect("Unable to build client... are we... out of sockets??")
        .start()
        .expect("Unable to start client")
}
