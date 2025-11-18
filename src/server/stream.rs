use bevy::ecs::component::Component;
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    common::stream::{
        QuicBidirectionalStreamAttempt, receive::QuicReceiveStream, send::QuicSendStream,
    },
    server::marker::QuicServerMarker,
};

#[derive(Component)]
#[require(QuicServerMarker)]
pub struct QuicServerBidirectionalStreamAttempt(QuicBidirectionalStreamAttempt);

impl QuicServerBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<
            Result<(QuicReceiveStream, QuicSendStream), s2n_quic::connection::Error>,
        >,
    ) -> Self {
        Self(QuicBidirectionalStreamAttempt::new(handle, conn_task))
    }
}
