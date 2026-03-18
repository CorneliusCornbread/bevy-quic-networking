use bevy::prelude::{Deref, DerefMut};
use tokio::{runtime::Handle, sync::oneshot, task::JoinHandle};

use crate::common::{
    attempt::{QuicActionAttempt, TaskError},
    stream::{receive::QuicReceiveStream, send::QuicSendStream},
};

pub mod disconnect;
pub mod id;
pub mod plugin;
pub mod receive;
pub mod send;
pub mod session;
pub mod task_state;

#[derive(Deref, DerefMut)]
pub struct QuicReceiveStreamAttempt(QuicActionAttempt<Option<QuicReceiveStream>>);

impl QuicReceiveStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: JoinHandle<Result<Option<QuicReceiveStream>, TaskError>>,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

#[derive(Deref, DerefMut)]
pub struct QuicSendStreamAttempt(QuicActionAttempt<QuicSendStream>);

impl QuicSendStreamAttempt {
    pub fn new(handle: Handle, conn_task: JoinHandle<Result<QuicSendStream, TaskError>>) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}

#[derive(Deref, DerefMut)]
pub struct QuicBidirectionalStreamAttempt(QuicActionAttempt<(QuicReceiveStream, QuicSendStream)>);

impl QuicBidirectionalStreamAttempt {
    pub fn new(
        handle: Handle,
        conn_task: oneshot::Receiver<Result<(QuicReceiveStream, QuicSendStream), TaskError>>,
    ) -> Self {
        Self(QuicActionAttempt::new(handle, conn_task))
    }
}
