use aeronet_io::SessionEndpoint;
use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::stream::{ReceiveStream, SendStream};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    client::marker::QuicClientMarker,
    common::{
        attempt::QuicActionError,
        connection::BidirectionalSessionAttempt,
        stream::{
            id::StreamId, receive::QuicReceiveStream, send::QuicSendStream,
            QuicBidirectionalStreamAttempt,
        },
    },
};

#[derive(Component)]
#[require(QuicClientMarker)]
#[require(SessionEndpoint)]
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

#[derive(Deref, DerefMut, Component)]
#[require(QuicClientMarker)]
pub struct QuicClientSendStream(QuicSendStream);

impl QuicClientSendStream {
    pub fn new(runtime: Handle, send: SendStream) -> Self {
        Self(QuicSendStream::new(runtime, send))
    }

    pub(crate) fn from_send_stream(quic_send: QuicSendStream) -> Self {
        Self(quic_send)
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicClientMarker)]
pub struct QuicClientReceiveStream(QuicReceiveStream);

impl QuicClientReceiveStream {
    pub fn new(runtime: Handle, rec: ReceiveStream) -> Self {
        Self(QuicReceiveStream::new(runtime, rec))
    }

    pub(crate) fn from_rec_stream(quic_rec: QuicReceiveStream) -> Self {
        Self(quic_rec)
    }
}
