use bevy::ecs::component::Component;
use s2n_quic::{Connection, connection::Error as ConnectionError};
use std::sync::Arc;
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinHandle},
};

use crate::common::connection::QuicConnection;

#[derive(Component)]
pub struct QuicConnectionAttempt {
    runtime: Handle,
    conn_task: Option<JoinHandle<Result<Connection, ConnectionError>>>,
    /// In the event that we have a failure with tokio, we store the error data here
    conn_join_error: Option<Arc<JoinError>>,
    conn_error: Option<ConnectionError>,
}

impl QuicConnectionAttempt {
    pub fn new(
        runtime: Handle,
        conn_task: JoinHandle<Result<Connection, ConnectionError>>,
    ) -> Self {
        Self {
            runtime,
            conn_task: Some(conn_task),
            conn_join_error: None,
            conn_error: None,
        }
    }

    pub fn get_connection(&mut self) -> Result<QuicConnection, ConnectionCreationError> {
        if let Some(e) = &self.conn_join_error {
            return Err(ConnectionCreationError::Crashed(e.clone()));
        }

        if let Some(e) = &self.conn_error {
            return Err(ConnectionCreationError::Failed(*e));
        }

        if self.conn_task.is_none() {
            return Err(ConnectionCreationError::AlreadyCreated);
        }

        let join_handle = self.conn_task.take().unwrap();

        if join_handle.is_finished() {
            let join_res = self.runtime.block_on(join_handle);

            if let Err(e) = join_res {
                let err_arc = Arc::new(e);
                self.conn_join_error = Some(err_arc.clone());
                return Err(ConnectionCreationError::Crashed(err_arc));
            }

            let res = join_res.unwrap();

            if let Err(e) = res {
                self.conn_error = Some(e.clone());
                return Err(ConnectionCreationError::Failed(e));
            }

            let conn = res.unwrap();

            return Ok(QuicConnection::new(self.runtime.clone(), conn));
        }

        Err(ConnectionCreationError::StillConnecting)
    }
}

/// The current status of the connection attempt
#[derive(Clone)]
pub enum ConnectionCreationError {
    /// Signals the connection is still in progress
    StillConnecting,
    /// Signals the connection has been completed and a session has already been created
    /// from a previous call
    AlreadyCreated,
    /// Signals the connection attempt has failed
    Failed(ConnectionError),
    /// Signals something has gone horribly wrong with the async runtime
    Crashed(Arc<JoinError>),
}
