use std::sync::Arc;

use bevy::ecs::{component::Component, system::Commands};
use s2n_quic::{
    Connection,
    connection::Error as ConnectionError,
    stream::{BidirectionalStream, Stream},
};
use tokio::{runtime::Handle, sync::Mutex};

use crate::common::{
    StreamId,
    stream::{attempt::QuicStreamAttempt, session::QuicSession},
};

pub mod attempt;

#[derive(Component)]
pub struct QuicConnection {
    runtime: Handle,
    connection: Arc<Mutex<Connection>>,
}

impl QuicConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            runtime,
            connection: Arc::new(Mutex::new(connection)),
        }
    }

    pub fn open_bidrectional_stream(&self, mut commands: Commands, id: StreamId) {
        let conn_task = self
            .runtime
            .spawn(bidirectional_task(self.connection.clone()));

        commands.spawn((
            QuicStreamAttempt::new(self.runtime.clone(), id, conn_task),
            QuicSession::new(id),
        ));
    }
}

async fn bidirectional_task(conn: Arc<Mutex<Connection>>) -> Result<Stream, ConnectionError> {
    let mut conn = conn.lock().await;

    match conn.open_bidirectional_stream().await {
        Ok(stream) => Ok(Stream::Bidirectional(stream)),
        Err(e) => Err(e),
    }
}
