use aeronet::io::SessionEndpoint;
use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::common::{
    attempt::QuicActionAttempt,
    stream::{receive::QuicReceiveStream, send::QuicSendStream},
};

pub mod id;
pub mod receive;
pub mod request;
pub mod send;
pub mod session;

#[derive(Deref, DerefMut, Component)]
#[require(SessionEndpoint)]
pub struct QuicReceiveStreamAttempt(QuicActionAttempt<QuicReceiveStream>);

impl QuicReceiveStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<QuicReceiveStream, s2n_quic::connection::Error>>,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(SessionEndpoint)]
pub struct QuicSendStreamAttempt(QuicActionAttempt<QuicSendStream>);

impl QuicSendStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<QuicSendStream, s2n_quic::connection::Error>>,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

#[derive(Deref, DerefMut, Component)]
#[require(SessionEndpoint)]
pub struct QuicBidirectionalStreamAttempt(QuicActionAttempt<(QuicReceiveStream, QuicSendStream)>);

impl QuicBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<
            Result<(QuicReceiveStream, QuicSendStream), s2n_quic::connection::Error>,
        >,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}
