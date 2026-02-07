use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::stream::{ReceiveStream, SendStream};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    common::{
        attempt::QuicActionError,
        stream::{
            QuicBidirectionalStreamAttempt, receive::QuicReceiveStream, send::QuicSendStream,
        },
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
        todo!();
        //Self(QuicBidirectionalStreamAttempt::new(handle, conn_task))
    }

    pub fn get_output(&mut self) -> Result<(QuicReceiveStream, QuicSendStream), QuicActionError> {
        todo!();
        //self.0.attempt_result()
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicServerMarker)]
pub struct QuicServerSendStream(QuicSendStream);

impl QuicServerSendStream {
    pub fn new(runtime: Handle, send: SendStream) -> Self {
        Self(QuicSendStream::new(runtime, send))
    }

    pub(crate) fn from_send_stream(quic_send: QuicSendStream) -> Self {
        Self(quic_send)
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicServerMarker)]
pub struct QuicServerReceiveStream(QuicReceiveStream);

impl QuicServerReceiveStream {
    pub fn new(runtime: Handle, rec: ReceiveStream) -> Self {
        Self(QuicReceiveStream::new(runtime, rec))
    }

    pub(crate) fn from_rec_stream(quic_rec: QuicReceiveStream) -> Self {
        Self(quic_rec)
    }
}
