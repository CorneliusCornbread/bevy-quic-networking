use std::sync::Arc;

use bevy::{
    ecs::{bundle::Bundle, component::Component},
    prelude::{Deref, DerefMut},
};
use s2n_quic::{Connection, connection::Error as ConnectionError, stream::BidirectionalStream};
use tokio::{runtime::Handle, sync::Mutex, task::JoinHandle};

use crate::common::{
    attempt::QuicActionAttempt,
    stream::{
        QuicBidirectionalStreamAttempt,
        id::{StreamId, StreamIdGenerator},
        receive::QuicReceiveStream,
        send::QuicSendStream,
        session::QuicSession,
    },
};

pub mod id;
pub mod plugin;
pub mod request;
pub mod runtime;

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
pub struct QuicConnectionAttempt(QuicActionAttempt<Connection>);

impl QuicConnectionAttempt {
    pub fn new(handle: Handle, conn_task: JoinHandle<Result<Connection, ConnectionError>>) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

#[derive(Debug, Component)]
pub struct QuicConnection {
    runtime: Handle,
    connection: Arc<Mutex<Connection>>,
    id_gen: StreamIdGenerator,
}

#[derive(Bundle)]
pub struct BidirectionalSessionAttempt(
    pub(crate) QuicBidirectionalStreamAttempt,
    pub(crate) QuicSession,
);

impl QuicConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            runtime,
            connection: Arc::new(Mutex::new(connection)),
            id_gen: Default::default(),
        }
    }

    pub(crate) fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

    pub(crate) fn open_bidrectional_stream(&mut self) -> BidirectionalSessionAttempt {
        let conn_task = self
            .runtime
            .spawn(open_bidirectional_task(self.connection.clone()));

        let id = self.id_gen.generate_id();

        BidirectionalSessionAttempt(
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), conn_task),
            QuicSession::new(id),
        )
    }

    pub(crate) fn generate_stream_id(&mut self) -> StreamId {
        self.id_gen.generate_id()
    }
}

async fn open_bidirectional_task(
    conn: Arc<Mutex<Connection>>,
) -> Result<(QuicReceiveStream, QuicSendStream), ConnectionError> {
    let bi_stream_res: Result<BidirectionalStream, ConnectionError>;

    {
        let mut conn = conn.lock().await;
        bi_stream_res = conn.open_bidirectional_stream().await;
    }

    let bi_stream = bi_stream_res?;
    let (rec, send) = bi_stream.split();

    let send_stream = QuicSendStream::new(Handle::current(), send);
    let rec_stream = QuicReceiveStream::new(Handle::current(), rec);

    Ok((rec_stream, send_stream))
}
