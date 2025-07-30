use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, atomic::AtomicBool},
};

use aeronet::io::SessionEndpoint;
use bevy::ecs::component::Component;
use s2n_quic::{
    Client, Connection,
    client::{Connect, ConnectionAttempt},
    connection::Error as ConnectionError,
};
use tokio::{
    runtime::Handle,
    sync::RwLock,
    task::{JoinError, JoinHandle},
};

use crate::session::QuicSession;

#[derive(Component)]
#[require(SessionEndpoint)]
pub struct QuicClientEndpoint {
    runtime: Handle,
    conn_task: Option<JoinHandle<Result<Connection, ConnectionError>>>,
    // In the event that we have a failure with tokio, we store the error data here
    join_error: Option<Arc<JoinError>>,
}

impl QuicClientEndpoint {
    pub fn connect(runtime: Handle, client: &Client, connect: Connect) -> Self {
        let conn = client.connect(connect);

        let conn_task = runtime.spawn(handle_connection(conn));

        Self {
            runtime,
            conn_task: Some(conn_task),
            join_error: None,
        }
    }

    pub fn get_session(&mut self) -> Result<QuicSession, SessionCreationError> {
        if let Some(e) = &self.join_error {
            return Err(SessionCreationError::Crashed(e.clone()));
        }

        if self.conn_task.is_none() {
            return Err(SessionCreationError::AlreadyCreated);
        }

        let join_handle = self.conn_task.take().unwrap();

        if join_handle.is_finished() {
            let join_res = self.runtime.block_on(join_handle);

            if let Err(e) = join_res {
                let err_arc = Arc::new(e);
                self.join_error.insert(err_arc.clone());
                return Err(SessionCreationError::Crashed(err_arc));
            }

            let res = join_res.unwrap();

            if let Err(e) = res {
                return Err(SessionCreationError::Failed(e));
            }

            let conn = res.unwrap();

            // TODO: split connections and streams into their own structures
            // we'll likely need a resource for this and the join handle
            // will likely be created externally instead.
            //let session = QuicSession::new(self.runtime.clone(), conn.id(), conn.acc)
        }

        Err(SessionCreationError::StillConnecting)
    }
}

async fn handle_connection(attempt: ConnectionAttempt) -> Result<Connection, ConnectionError> {
    attempt.await
}

/// The current status of the connection attempt
#[derive(Clone)]
enum SessionCreationError {
    /// Signals the endpoint is still in progress
    StillConnecting,
    /// Signals the connection has been completed and a session has already been created
    /// from a previous call
    AlreadyCreated,
    /// Signals the connection attempt has failed
    Failed(ConnectionError),
    /// Signals something has gone horribly wrong with the async runtime
    Crashed(Arc<JoinError>),
}
