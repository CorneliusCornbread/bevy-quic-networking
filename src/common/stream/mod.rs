use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::stream::PeerStream;
use tokio::runtime::Handle;

use crate::common::{
    QuicParentId,
    attempt::{QuicActionAttempt, TaskResult},
    stream::{receive::QuicReceiveStream, send::QuicSendStream},
};

pub mod disconnect;
pub mod id;
pub mod plugin;
pub mod receive;
pub mod send;
pub mod session;
pub mod task_state;

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
pub struct QuicReceiveStreamAttempt(QuicActionAttempt<Option<QuicReceiveStream>>);

impl QuicReceiveStreamAttempt {
    pub fn new(
        handle: Handle,
        task: impl TaskResult<Option<QuicReceiveStream>> + 'static + Send + Sync,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, task, parent_id))
    }
}

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
pub struct QuicSendStreamAttempt(QuicActionAttempt<Option<QuicSendStream>>);

impl QuicSendStreamAttempt {
    pub fn new(
        handle: Handle,
        task: impl TaskResult<Option<QuicSendStream>> + 'static + Send + Sync,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, task, parent_id))
    }
}

#[derive(Deref, DerefMut, Component)]
#[component(storage = "SparseSet")]
pub struct QuicBidirectionalStreamAttempt(
    QuicActionAttempt<Option<(QuicReceiveStream, QuicSendStream)>>,
);

impl QuicBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        task: impl TaskResult<Option<(QuicReceiveStream, QuicSendStream)>> + 'static + Send + Sync,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, task, parent_id))
    }
}

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct QuicPeerStreamAttempt(QuicActionAttempt<Option<QuicPeerStream>>);

impl QuicPeerStreamAttempt {
    pub fn new(
        handle: Handle,
        task: impl TaskResult<Option<QuicPeerStream>> + 'static + Send + Sync,
        parent_id: QuicParentId,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, task, parent_id))
    }
}

pub enum QuicPeerStream {
    Bidirectional(QuicReceiveStream, QuicSendStream),
    Receive(QuicReceiveStream),
}

impl QuicPeerStream {
    pub fn new(runtime: Handle, peer_stream: PeerStream, parent_id: QuicParentId) -> Self {
        match peer_stream {
            PeerStream::Bidirectional(bidirectional_stream) => {
                let (rec, send) = bidirectional_stream.split();
                let quic_rec = QuicReceiveStream::new(runtime.clone(), rec, parent_id);
                let quic_send = QuicSendStream::new(runtime, send, parent_id);

                QuicPeerStream::Bidirectional(quic_rec, quic_send)
            }
            PeerStream::Receive(rec) => {
                let quic_rec = QuicReceiveStream::new(runtime.clone(), rec, parent_id);

                QuicPeerStream::Receive(quic_rec)
            }
        }
    }
}
