use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::stream::{ReceiveStream, SendStream};
use tokio::{runtime::Handle, sync::oneshot};

use crate::{
    common::{
        QuicParentId,
        attempt::{QuicActionError, TaskError},
        stream::{BidirectionalStreamAttempt, receive::QuicReceiveStream, send::QuicSendStream},
    },
    server::marker::QuicServerMarker,
};

#[derive(Component)]
#[require(QuicServerMarker)]
pub struct QuicServerBidirectionalStreamAttempt(BidirectionalStreamAttempt);

impl QuicServerBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: oneshot::Receiver<Result<(QuicReceiveStream, QuicSendStream), TaskError>>,
        parent_id: QuicParentId,
    ) -> Self {
        Self(BidirectionalStreamAttempt::new(
            handle, conn_task, parent_id,
        ))
    }

    pub fn get_output(&mut self) -> Result<(QuicReceiveStream, QuicSendStream), QuicActionError> {
        self.0.attempt_result()
    }

    pub fn parent_id(&self) -> QuicParentId {
        self.0.parent_id()
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicServerMarker)]
pub struct QuicServerSendStream(QuicSendStream);

impl QuicServerSendStream {
    pub fn new(runtime: Handle, send: SendStream, parent_id: QuicParentId) -> Self {
        Self(QuicSendStream::new(runtime, send, parent_id))
    }

    pub(crate) fn from_send_stream(quic_send: QuicSendStream) -> Self {
        Self(quic_send)
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicServerMarker)]
pub struct QuicServerReceiveStream(QuicReceiveStream);

impl QuicServerReceiveStream {
    pub fn new(runtime: Handle, rec: ReceiveStream, parent_id: QuicParentId) -> Self {
        Self(QuicReceiveStream::new(runtime, rec, parent_id))
    }

    pub(crate) fn from_rec_stream(quic_rec: QuicReceiveStream) -> Self {
        Self(quic_rec)
    }
}
