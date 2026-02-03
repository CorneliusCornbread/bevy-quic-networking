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

    // TODO: add default system to accept all incoming streams for clients
    /// Called to accept any pending streams manually. Should only be done
    /// if you're using a plugin setup which doesn't use the default accepters.
    pub fn accept_streams(&mut self) -> Result<(PeerStream, StreamId), StreamPollError> {
        self.connection.accept_streams()
    }

    /// Requests to open a bidirectional stream on the current client connection.
    ///
    /// Returns a tuple including the stream attempt and a unique StreamId.
    pub fn open_bidrectional_stream(&mut self) -> (QuicClientBidirectionalStreamAttempt, StreamId) {
        QuicClientBidirectionalStreamAttempt::from_session_attempt(
            self.connection.open_bidrectional_stream(),
        )
    }

    // TODO: add open unidirectional stream function

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
