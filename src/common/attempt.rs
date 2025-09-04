use std::sync::Arc;

use aeronet::io::SessionEndpoint;
use bevy::ecs::component::Component;
use s2n_quic::connection::Error as ConnectionError;
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinHandle},
};

// TODO: Figure out how we want to structure endpoints in regards to these attempts
// atm both streams and connectionss would have endpoints which doesn't make sense.
// This would also make it look like there's a constant endpoint pending a connection.
#[derive(Component)]
#[require(SessionEndpoint)]
pub struct QuicActionAttempt<T> {
    runtime: Handle,
    conn_task: Option<JoinHandle<Result<T, ConnectionError>>>,
    /// In the event that we have a failure with tokio, we store the error data here
    last_error: Option<QuicActionError>,
}

impl<T> QuicActionAttempt<T> {
    pub fn new(handle: Handle, conn_task: JoinHandle<Result<T, ConnectionError>>) -> Self {
        Self {
            runtime: handle,
            conn_task: Some(conn_task),
            last_error: None,
        }
    }

    pub fn get_output(&mut self) -> Result<T, QuicActionError> {
        if let Some(e) = &self.last_error {
            return Err(e.clone());
        }

        let join_handle_res = self.conn_task.take();

        if join_handle_res.is_none() {
            return Err(QuicActionError::Consumed);
        }

        let join_handle = join_handle_res.unwrap();

        if !join_handle.is_finished() {
            return Err(QuicActionError::InProgress);
        }

        let join_res = self.runtime.block_on(join_handle);

        if let Err(e) = join_res {
            let err_arc = Arc::new(e);
            let creation_err = QuicActionError::Crashed(err_arc.clone());
            self.last_error = Some(creation_err.clone());
            return Err(creation_err);
        }

        let res = join_res.unwrap();

        if let Err(e) = res {
            let creation_err = QuicActionError::Failed(e);
            self.last_error = Some(creation_err.clone());
            return Err(creation_err);
        }

        Ok(res.unwrap())
    }
}

#[derive(Clone, Debug)]
pub enum QuicActionError {
    InProgress,
    Consumed,
    Failed(ConnectionError),
    Crashed(Arc<JoinError>),
}
