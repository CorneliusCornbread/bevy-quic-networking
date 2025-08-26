use std::sync::Arc;

use bevy::{
    ecs::{bundle::Bundle, component::Component},
    prelude::Deref,
};
use s2n_quic::{Connection, connection::Error as ConnectionError, stream::BidirectionalStream};
use tokio::{runtime::Handle, sync::Mutex};

use crate::common::{
    attempt::QuicActionAttempt,
    stream::{
        QuicBidirectionalStreamAttempt, StreamId, receive::QuicReceiveStream, send::QuicSendStream,
        session::QuicSession,
    },
};

pub type QuicConnectionAttempt = QuicActionAttempt<Connection, ConnectionId>;

#[derive(Component)]
pub struct QuicConnection {
    runtime: Handle,
    connection: Arc<Mutex<Connection>>,
}

// TODO: Okay I'm feeling like this approach is wrong.
// I think this can be done better by instead constructing our stream attempts
// and passing in the connection during construction which might lend itself
// better to the ECS design.
// As opposed to a connection that spawns streams...
// I dunno, maybe, maybe not. I'm gonna sleep on it.
#[derive(Bundle)]
pub struct BidirectionalSessionAttempt(QuicBidirectionalStreamAttempt, QuicSession);

impl QuicConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            runtime,
            connection: Arc::new(Mutex::new(connection)),
        }
    }

    pub(crate) fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

    pub(crate) fn open_bidrectional_stream(&self, id: StreamId) -> BidirectionalSessionAttempt {
        let conn_task = self
            .runtime
            .spawn(open_bidirectional_task(self.connection.clone()));

        BidirectionalSessionAttempt(
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), id, conn_task),
            QuicSession::new(id),
        )
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

#[derive(Deref, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ConnectionId(u64);

impl From<u64> for ConnectionId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
