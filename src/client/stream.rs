use bevy::ecs::component::Component;
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    client::marker::QuicClientMarker,
    common::{
        attempt::QuicActionError,
        connection::BidirectionalSessionAttempt,
        stream::{
            QuicBidirectionalStreamAttempt, id::StreamId, receive::QuicReceiveStream,
            send::QuicSendStream,
        },
    },
};

#[derive(Component)]
#[require(QuicClientMarker)]
pub struct QuicClientBidirectionalStreamAttempt(QuicBidirectionalStreamAttempt);

impl QuicClientBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<
            Result<(QuicReceiveStream, QuicSendStream), s2n_quic::connection::Error>,
        >,
    ) -> Self {
        Self(QuicBidirectionalStreamAttempt::new(handle, conn_task))
    }

    pub fn get_output(&mut self) -> Result<(QuicReceiveStream, QuicSendStream), QuicActionError> {
        self.0.get_output()
    }

    pub(crate) fn from_session_attempt(attempt: BidirectionalSessionAttempt) -> (Self, StreamId) {
        (Self(attempt.0), attempt.1)
    }
}
