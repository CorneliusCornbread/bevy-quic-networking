use std::{error::Error, sync::Arc, time::SystemTime};

use bevy::ecs::component::Component;
use s2n_quic::connection::Error as ConnectionError;
use thiserror::Error as ThisError;
use tokio::{
    runtime::Handle,
    sync::oneshot::{self, error::TryRecvError},
    task::JoinHandle,
};

pub trait TaskResult<T> {
    fn resolve_result(&mut self, handle: &Handle) -> Option<Result<T, TaskError>>;
}

#[derive(ThisError, Debug)]
#[error("The send channel was closed without sending a value.")]
pub struct ChannelClosed;

impl<T> TaskResult<T> for oneshot::Receiver<Result<T, TaskError>> {
    fn resolve_result(&mut self, _handle: &Handle) -> Option<Result<T, TaskError>> {
        if self.is_empty() {
            return None;
        }

        match self.try_recv() {
            Ok(res) => Some(res),
            Err(e) => match e {
                TryRecvError::Empty => None,
                TryRecvError::Closed => Some(Err(TaskError::TaskFailed(Arc::new(e)))),
            },
        }
    }
}

impl<T> TaskResult<T> for JoinHandle<Result<T, TaskError>> {
    fn resolve_result(&mut self, handle: &Handle) -> Option<Result<T, TaskError>> {
        if !self.is_finished() {
            return None;
        }

        let join = handle.block_on(self);

        match join {
            Ok(res) => Some(res),
            Err(e) => Some(Err(TaskError::TaskFailed(Arc::new(e)))),
        }
    }
}

pub struct QuicActionAttempt<T> {
    pub(crate) runtime: Handle,
    pub(crate) task_res: Box<dyn TaskResult<T> + Send + Sync>,
    /// A flag checking if the action state has returned a success value already
    pub(crate) returned_value: Option<QuicActionError>,
}

impl<T> QuicActionAttempt<T> {
    pub fn new(runtime: Handle, task: impl TaskResult<T> + 'static + Send + Sync) -> Self {
        Self {
            runtime,
            task_res: Box::new(task),
            returned_value: None,
        }
    }

    pub fn attempt_result(&mut self) -> Result<T, QuicActionError> {
        if let Some(ret) = &self.returned_value {
            return Err(ret.clone());
        }

        let value = self.task_res.resolve_result(&self.runtime);

        let Some(res) = value else {
            return Err(QuicActionError::Pending);
        };

        match res {
            Ok(value) => {
                self.returned_value = Some(QuicActionError::Consumed);
                Ok(value)
            }
            Err(e) => match e {
                TaskError::ConnectionFailed(err) => {
                    self.returned_value = Some(QuicActionError::ConnectionFailed(err));
                    Err(self.returned_value.as_ref().unwrap().clone())
                }
                TaskError::TaskFailed(err) => {
                    self.returned_value = Some(QuicActionError::Crashed(Arc::new(err)));
                    Err(self.returned_value.as_ref().unwrap().clone())
                }
            },
        }
    }
}

#[derive(Clone, Debug, ThisError)]
#[error("Quic action attempt failed to get a result")]
pub enum QuicActionError {
    #[error("Pending")]
    Pending,
    #[error("Consumed")]
    Consumed,
    #[error("ConnectionFailed: {0}")]
    ConnectionFailed(ConnectionError),
    #[error("Crashed: {0}")]
    Crashed(Arc<dyn std::error::Error + Send + Sync>),
}

#[derive(Clone, Debug, ThisError)]
#[error("Quic task failed")]
pub enum TaskError {
    #[error("ConnectionFailed: {0}")]
    ConnectionFailed(ConnectionError),
    #[error("Crashed: {0}")]
    TaskFailed(Arc<dyn Error + Send + Sync>),
}

#[derive(Component, Debug, Clone)]
pub struct QuicActionErrorComponent {
    error: QuicActionError,
    timestamp: SystemTime,
}

impl QuicActionErrorComponent {
    pub fn new(error: QuicActionError, timestamp: SystemTime) -> Self {
        Self { error, timestamp }
    }

    pub fn error(&self) -> &QuicActionError {
        &self.error
    }

    pub fn timestamp(&self) -> &SystemTime {
        &self.timestamp
    }
}
