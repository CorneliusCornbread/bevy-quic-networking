use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError, stream::PeerStream};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    client::{marker::QuicClientMarker, stream::QuicClientBidirectionalStreamAttempt},
    common::{
        connection::{QuicConnection, QuicConnectionAttempt, StreamPollError},
        stream::id::StreamId,
    },
};

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
#[require(QuicClientMarker)]
pub struct QuicClientConnectionAttempt(QuicConnectionAttempt);

impl QuicClientConnectionAttempt {
    pub fn new(handle: Handle, conn_task: JoinHandle<Result<Connection, ConnectionError>>) -> Self {
        Self(QuicConnectionAttempt::new(handle, conn_task))
    }
}

#[derive(Debug, Component)]
#[require(QuicClientMarker)]
pub struct QuicClientConnection {
    connection: QuicConnection,
}

impl QuicClientConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            connection: QuicConnection::new(runtime, connection),
        }
    }

    pub(crate) fn from_connection(connection: QuicConnection) -> Self {
        Self { connection }
    }

    pub fn accept_streams(&mut self) -> Result<(PeerStream, StreamId), StreamPollError> {
        self.connection.accept_streams()
    }

    pub fn open_bidrectional_stream(&mut self) -> (QuicClientBidirectionalStreamAttempt, StreamId) {
        QuicClientBidirectionalStreamAttempt::from_session_attempt(
            self.connection.open_bidrectional_stream(),
        )
    }
}
