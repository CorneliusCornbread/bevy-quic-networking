use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    common::connection::{QuicConnection, QuicConnectionAttempt},
    server::marker::QuicServerMarker,
};

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
#[require(QuicServerMarker)]
pub struct QuicServerConnectionAttempt(QuicConnectionAttempt);

impl QuicServerConnectionAttempt {
    pub fn new(handle: Handle, conn_task: JoinHandle<Result<Connection, ConnectionError>>) -> Self {
        Self(QuicConnectionAttempt::new(handle, conn_task))
    }
}

#[derive(Debug, Component, Deref, DerefMut)]
#[require(QuicServerMarker)]
pub struct QuicServerConnection {
    connection: QuicConnection,
}

// TODO: add open bidirectional stream and unidirectional stream functions
impl QuicServerConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            connection: QuicConnection::new(runtime, connection),
        }
    }

    pub(crate) fn from_connection(connection: QuicConnection) -> Self {
        QuicServerConnection { connection }
    }

    /// Returns true if the connection is still open.
    pub fn is_open(&mut self) -> bool {
        self.connection.is_open()
    }

    /// Gets the disconnect reason if the stream has closed.
    /// Returns `None` if the stream is still open.
    pub fn get_disconnect_reason(&mut self) -> Option<ConnectionError> {
        self.connection.get_disconnect_reason()
    }
}
