use std::sync::Arc;

use bevy::ecs::component::Component;
use s2n_quic::{
    Connection, Server, client::ConnectionAttempt, connection::Error as ConnectionError,
};
use tokio::{
    runtime::Handle,
    sync::Mutex,
    task::{JoinError, JoinHandle},
};

use crate::common::connection::{QuicConnection, id::ConnectionIdGenerator};

pub mod endpoint;
//pub mod flag;

#[derive(Component)]
pub struct QuicServer {
    runtime: Handle,
    server: Arc<Mutex<Server>>,
    id_gen: ConnectionIdGenerator,
    poll_job: Option<JoinHandle<Option<Connection>>>,
}

impl QuicServer {
    pub fn new(runtime: Handle, server: Server) -> Self {
        Self {
            runtime,
            server: Arc::new(Mutex::new(server)),
            id_gen: Default::default(),
            poll_job: None,
        }
    }

    pub fn poll_connection(&mut self) -> Result<ConnectionPoll, JoinError> {
        if self.poll_job.is_none() {
            let poll = self.runtime.spawn(poll_connection(self.server.clone()));
            self.poll_job = Some(poll);
            return Ok(ConnectionPoll::None);
        }

        let job = self.poll_job.as_mut().unwrap();

        if !job.is_finished() {
            return Ok(ConnectionPoll::None);
        }

        let conn_opt = self.runtime.block_on(job)?;

        if let Some(conn) = conn_opt {
            return Ok(ConnectionPoll::NewConnection(QuicConnection::new(
                self.runtime.clone(),
                conn,
            )));
        }

        Ok(ConnectionPoll::ServerClosed)
    }

    /// Starts a connection poll if one is not already running.
    /// Returns Some(()) if no poll was running and one was successfully started.
    /// Returns None if there's already a running poll.
    pub fn start_connection_poll(&mut self) -> Option<()> {
        todo!()
    }
}

#[derive(Debug)]
pub enum ConnectionPoll {
    None,
    ServerClosed,
    NewConnection(QuicConnection),
}

async fn create_connection(attempt: ConnectionAttempt) -> Result<Connection, ConnectionError> {
    attempt.await
}

async fn poll_connection(server: Arc<Mutex<Server>>) -> Option<Connection> {
    let mut lock = server.lock().await;
    lock.accept().await
}
