use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError, stream::PeerStream};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    client::{
        marker::QuicClientMarker,
        stream::{
            QuicClientBidirectionalStreamAttempt, QuicClientReceiveStream,
            QuicClientSendStreamAttempt,
        },
    },
    common::{
        QuicParentId,
        attempt::TaskError,
        connection::{
            ConnectionCommandError, QuicConnection, QuicConnectionAttempt,
            disconnect::ConnectionDisconnectReason,
        },
    },
};

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
#[require(QuicClientMarker)]
pub struct QuicClientConnectionAttempt(QuicConnectionAttempt);

impl QuicClientConnectionAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<Connection, TaskError>>,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicConnectionAttempt::new(handle, conn_task, parent_id))
    }
}

#[derive(Debug, Component)]
#[require(QuicClientMarker)]
pub struct QuicClientConnection {
    connection: QuicConnection,
}

impl QuicClientConnection {
    pub fn new(runtime: Handle, connection: Connection, parent_id: QuicParentId) -> Self {
        Self {
            connection: QuicConnection::new(runtime, connection, parent_id),
        }
    }

    pub(crate) fn from_connection(connection: QuicConnection) -> Self {
        Self { connection }
    }

    /// Called to accept any pending streams manually. Should only be done
    /// if you're using a plugin setup which doesn't use the default accepters.
    //TODO: make our own "PeerStream" enum with the parent Id embedded
    pub fn accept_streams(&mut self) -> Result<(PeerStream, QuicParentId), ConnectionCommandError> {
        self.connection.accept_streams()
    }

    /// Requests to open a bidirectional stream on the current client connection.
    ///
    /// Returns a tuple including the stream attempt and a unique StreamId.
    pub fn open_bidrectional_stream(&mut self) -> QuicClientBidirectionalStreamAttempt {
        QuicClientBidirectionalStreamAttempt::from_attempt(
            self.connection.open_bidrectional_stream().unwrap(),
        )
    }

    pub fn open_send_stream(&mut self) -> QuicClientSendStreamAttempt {
        todo!()
    }

    pub fn accept_receive_stream(&mut self) -> Option<QuicClientReceiveStream> {
        let rec = self.connection.accept_receive_stream();

        todo!()
    }

    /// Returns true if the connection is still open.
    pub fn is_open(&mut self) -> bool {
        self.connection.is_open()
    }

    /// Gets the disconnect reason if the stream has closed.
    /// Returns `None` if the stream is still open.
    pub fn get_disconnect_reason(&mut self) -> Option<ConnectionDisconnectReason> {
        self.connection.get_disconnect_reason()
    }
}
