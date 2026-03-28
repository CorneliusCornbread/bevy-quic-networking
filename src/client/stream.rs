use aeronet_io::SessionEndpoint;
use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::stream::{ReceiveStream, SendStream};
use tokio::{runtime::Handle, sync::oneshot, task::JoinHandle};

use crate::{
    client::marker::QuicClientMarker,
    common::{
        QuicParentId,
        attempt::{QuicActionError, TaskError},
        connection::BidirectionalSessionAttempt,
        stream::{
            BidirectionalStreamAttempt, QuicSendStreamAttempt, receive::QuicReceiveStream,
            send::QuicSendStream,
        },
    },
};

#[derive(Component)]
#[require(QuicClientMarker)]
#[require(SessionEndpoint)]
pub struct QuicClientBidirectionalStreamAttempt(BidirectionalStreamAttempt);

impl QuicClientBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: oneshot::Receiver<Result<(QuicReceiveStream, QuicSendStream), TaskError>>,
        parent_id: QuicParentId,
    ) -> Self {
        Self(BidirectionalStreamAttempt::new(
            handle, conn_task, parent_id,
        ))
    }

    pub fn attempt_result(
        &mut self,
    ) -> Result<(QuicReceiveStream, QuicSendStream), QuicActionError> {
        self.0.attempt_result()
    }

    pub(crate) fn from_session_attempt(attempt: BidirectionalSessionAttempt) -> Self {
        Self(attempt.0)
    }

    pub fn parent_id(&self) -> QuicParentId {
        self.0.parent_id()
    }
}

#[derive(Component)]
#[require(QuicClientMarker)]
#[require(SessionEndpoint)]
pub struct QuicClientSendStreamAttempt(QuicSendStreamAttempt);

impl QuicClientSendStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<QuicSendStream, TaskError>>,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicSendStreamAttempt::new(handle, conn_task, parent_id))
    }

    pub fn attempt_result(&mut self) -> Result<QuicSendStream, QuicActionError> {
        self.0.attempt_result()
    }

    pub fn parent_id(&self) -> QuicParentId {
        self.0.parent_id()
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicClientMarker)]
pub struct QuicClientSendStream(QuicSendStream);

impl QuicClientSendStream {
    pub fn new(runtime: Handle, send: SendStream, parent_id: QuicParentId) -> Self {
        Self(QuicSendStream::new(runtime, send, parent_id))
    }

    pub(crate) fn from_send_stream(quic_send: QuicSendStream) -> Self {
        Self(quic_send)
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(QuicClientMarker)]
pub struct QuicClientReceiveStream(QuicReceiveStream);

impl QuicClientReceiveStream {
    pub fn new(runtime: Handle, rec: ReceiveStream, parent_id: QuicParentId) -> Self {
        Self(QuicReceiveStream::new(runtime, rec, parent_id))
    }

    pub(crate) fn from_rec_stream(quic_rec: QuicReceiveStream) -> Self {
        Self(quic_rec)
    }
}
