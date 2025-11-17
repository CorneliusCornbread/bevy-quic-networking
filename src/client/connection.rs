use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::common::connection::{QuicConnection, QuicConnectionAttempt};

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
pub struct QuicClientConnectionAttempt(QuicConnectionAttempt);

impl QuicClientConnectionAttempt {
    pub fn new(handle: Handle, conn_task: JoinHandle<Result<Connection, ConnectionError>>) -> Self {
        Self(QuicConnectionAttempt::new(handle, conn_task))
    }
}

#[derive(Debug, Component, Deref, DerefMut)]
pub struct QuicClientConnection {
    connection: QuicConnection,
}

impl QuicClientConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            connection: QuicConnection::new(runtime, connection),
        }
    }
}
