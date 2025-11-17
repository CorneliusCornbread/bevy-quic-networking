use std::{error::Error, sync::Arc};

use bevy::{
    ecs::{bundle::Bundle, component::Component},
    prelude::{Deref, DerefMut},
};
use s2n_quic::{
    Connection,
    connection::Error as ConnectionError,
    stream::{BidirectionalStream, PeerStream},
};
use tokio::{runtime::Handle, sync::Mutex, task::JoinHandle};

use crate::common::{
    attempt::QuicActionAttempt,
    stream::{
        QuicBidirectionalStreamAttempt,
        id::{StreamId, StreamIdGenerator},
        receive::QuicReceiveStream,
        send::QuicSendStream,
    },
};

pub mod id;
pub mod plugin;
pub mod request;
pub mod runtime;

#[derive(Deref, DerefMut)]
pub struct QuicConnectionAttempt(QuicActionAttempt<Connection>);

impl QuicConnectionAttempt {
    pub(crate) fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<Connection, ConnectionError>>,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

#[derive(Debug)]
pub struct QuicConnection {
    runtime: Handle,
    connection: Arc<Mutex<Connection>>,
    id_gen: StreamIdGenerator,
}

#[derive(Bundle)]
pub struct BidirectionalSessionAttempt(pub QuicBidirectionalStreamAttempt, pub StreamId);

impl QuicConnection {
    pub fn new(runtime: Handle, mut connection: Connection) -> Self {
        connection
            .keep_alive(true)
            .expect("Unable to keep alive connection");
        Self {
            runtime,
            connection: Arc::new(Mutex::new(connection)),
            id_gen: Default::default(),
        }
    }

    pub fn accept_streams(&mut self) -> Result<(PeerStream, StreamId), StreamPollError> {
        let waker = Arc::new(futures::task::noop_waker_ref());
        let mut cx = std::task::Context::from_waker(&waker);

        let mut lock = self.connection.try_lock()?;
        let poll = lock.poll_accept(&mut cx);

        if let std::task::Poll::Ready(res) = poll
            && let Ok(opt) = res
            && let Some(stream) = opt
        {
            return Ok((stream, self.id_gen.generate_id()));
        }

        Err(StreamPollError::None)
    }

    pub(crate) fn open_bidrectional_stream(&mut self) -> BidirectionalSessionAttempt {
        let conn_task = self
            .runtime
            .spawn(open_bidirectional_task(self.connection.clone()));

        BidirectionalSessionAttempt(
            QuicBidirectionalStreamAttempt::new(self.runtime.clone(), conn_task),
            self.generate_stream_id(),
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

#[derive(Debug)]
pub enum StreamPollError {
    None,
    Error(Box<dyn Error>),
}

impl<E> From<E> for StreamPollError
where
    E: Error + 'static,
{
    fn from(err: E) -> Self {
        StreamPollError::Error(Box::new(err))
    }
}
