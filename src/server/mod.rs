use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::ecs::component::Component;
use s2n_quic::{
    Connection, Server, client::ConnectionAttempt, connection::Error as ConnectionError,
};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{common::connection::id::ConnectionIdGenerator, server::flag::AtomicPollFlag};

pub mod endpoint;
pub mod flag;

#[derive(Component)]
pub struct QuicServer {
    runtime: Handle,
    server: Server,
    id_gen: ConnectionIdGenerator,
    poll_flag: Arc<AtomicPollFlag>,
    poll_job: Option<JoinHandle<()>>,
}

impl QuicServer {
    pub fn new(runtime: Handle, server: Server) -> Self {
        Self {
            runtime,
            server,
            id_gen: Default::default(),
            poll_flag: Arc::new(AtomicPollFlag::new(flag::PollState::Stopped)),
            poll_job: None,
        }
    }

    pub fn stop_poll(&self) {
        match self.poll_flag.load(Ordering::Relaxed) {
            flag::PollState::Stopped | flag::PollState::Stopping => {
                return;
            }
            _ => (),
        }
    }

    pub fn start_poll(&self) {}
}

async fn create_connection(attempt: ConnectionAttempt) -> Result<Connection, ConnectionError> {
    attempt.await
}
