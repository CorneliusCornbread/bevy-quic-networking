use aeronet::io::SessionEndpoint;
use bevy::ecs::component::Component;
use s2n_quic::{connection::Error as ConnectionError, stream::Stream};
use std::sync::Arc;
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinHandle},
};

use crate::common::StreamId;

#[derive(Component)]
#[require(SessionEndpoint)]
pub struct QuicStreamAttempt {
    runtime: Handle,
    conn_task: Option<JoinHandle<Result<Stream, ConnectionError>>>,
    /// In the event that we have a failure with tokio, we store the error data here
    conn_join_error: Option<Arc<JoinError>>,
    stream_id: StreamId,
}

impl QuicStreamAttempt {
    pub fn new(
        handle: Handle,
        stream_id: StreamId,
        conn_task: JoinHandle<Result<Stream, ConnectionError>>,
    ) -> Self {
        Self {
            runtime: handle,
            conn_task: Some(conn_task),
            conn_join_error: None,
            stream_id,
        }
    }
}
