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
    stream::{
        QuicBidirectionalStreamAttempt, QuicSendStreamAttempt, receive::QuicReceiveStream,
        send::QuicSendStream, session::QuicSession,
    },
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
            .spawn(open_bidirectional_task(self.connection.clone()));

        commands.spawn((
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), id, conn_task),
            QuicSession::new(id),
        ));
    }
}

async fn open_bidirectional_task(
    conn: Arc<Mutex<Connection>>,
) -> Result<(QuicReceiveStream, QuicSendStream), ConnectionError> {
    let bi_stream_res: Result<BidirectionalStream, ConnectionError>;

    {
        let mut conn = conn.lock().await;
        bi_stream_res = conn.open_bidirectional_stream().await
    }

    let bi_stream = bi_stream_res?;
    let (rec, send) = bi_stream.split();

    let send_stream = QuicSendStream::new(Handle::current(), send);
    let rec_stream = QuicReceiveStream::new(Handle::current(), rec);

    Ok((rec_stream, send_stream))
}
