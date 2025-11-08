use std::sync::Arc;

use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
    connection::Error as ConnectionError,
};
use s2n_quic_tls::{certificate::IntoCertificate, error::Error as TlsError};
use tokio::{runtime::Handle, sync::Mutex};

use crate::{
    client::marker::QuicClientMarker,
    common::connection::{
        QuicConnectionAttempt,
        id::{ConnectionId, ConnectionIdGenerator},
        runtime::TokioRuntime,
    },
};

pub mod marker;

#[derive(Component)]
#[require(QuicClientMarker)]
pub struct QuicClient {
    runtime: Handle,
    client: Arc<Mutex<Client>>,
    id_gen: ConnectionIdGenerator,
}

impl QuicClient {
    pub fn new(runtime: &TokioRuntime) -> Self {
        let client = runtime.block_on(build());

        Self {
            runtime: runtime.handle().clone(),
            client: Arc::new(Mutex::new(client)),
            id_gen: Default::default(),
        }
    }

    pub fn new_with_tls<C: IntoCertificate>(
        runtime: &TokioRuntime,
        certificate: C,
    ) -> Result<Self, TlsError> {
        let client = runtime.block_on(build_tls(certificate))?;

        let ret = Self {
            runtime: runtime.handle().clone(),
            client: Arc::new(Mutex::new(client)),
            id_gen: Default::default(),
        };

        Ok(ret)
    }

    pub(crate) fn open_connection(
        &mut self,
        connect: Connect,
    ) -> (QuicConnectionAttempt, ConnectionId, QuicClientMarker) {
        let client = &self.client.blocking_lock();
        let attempt = client.connect(connect);

        let conn_task = self.runtime.spawn(create_connection(attempt));

        (
            QuicConnectionAttempt::new(self.runtime.clone(), conn_task),
            self.id_gen.generate_id(),
            Default::default(),
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

async fn build_tls<C: IntoCertificate>(certificate: C) -> Result<Client, TlsError> {
    let tls = s2n_quic_tls::Client::builder()
        .with_certificate(certificate)?
        .build()?;

    let client = Client::builder()
        .with_io("0.0.0.0:0")
        .expect("Unable to build client... are we... out of sockets??")
        .with_tls(tls)
        .expect("Invalid TLS")
        .start()
        .expect("Unable to start client");

    Ok(client)
}
