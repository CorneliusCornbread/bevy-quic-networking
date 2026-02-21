use std::{error::Error, mem, sync::Arc, time::SystemTime};

use bevy::ecs::component::Component;
use s2n_quic::connection::Error as ConnectionError;
use tokio::{
    runtime::Handle,
    sync::oneshot,
    task::{JoinError, JoinHandle},
};

pub trait TaskResult<T>: Send {
    fn resolve_result(
        &mut self,
        handle: &Handle,
    ) -> Option<Result<T, Box<dyn Error + Send + Sync>>>;
}

impl<T: Send> TaskResult<T> for oneshot::Receiver<T> {
    fn resolve_result(
        &mut self,
        _handle: &Handle,
    ) -> Option<Result<T, Box<dyn Error + Send + Sync>>> {
        if self.is_empty() {
            return None;
        }

        match self.try_recv() {
            Ok(out) => Some(Ok(out)),
            Err(e) => Some(Err(Box::new(e))),
        }
    }
}

impl<T: Send> TaskResult<T> for JoinHandle<T> {
    fn resolve_result(
        &mut self,
        handle: &Handle,
    ) -> Option<Result<T, Box<dyn Error + Send + Sync>>> {
        if !self.is_finished() {
            return None;
        }

        match handle.block_on(self) {
            Ok(out) => Some(Ok(out)),
            Err(e) => Some(Err(Box::new(e))),
        }
    }
}

#[derive(Debug)]
enum ActionPoll<T, A: TaskResult<T>> {
    /// Task is still running
    Pending(A),
    /// Task completed successfully
    Completed(T),
    /// Task failed with an error
    Failed(Box<dyn Error + Send + Sync>),
    /// Invalid state, only used for moving between states
    Invalid,
}

pub(crate) struct QuicActionAttempt<T, A: TaskResult<T>> {
    pub(crate) runtime: Handle,
    pub(crate) action_state: ActionPoll<T, A>,
}

impl<T, A: TaskResult<T>> QuicActionAttempt<T, A> {
    fn attempt_result(&mut self) -> Result<T, QuicActionError> {
        let mut state: ActionPoll<T, A> = ActionPoll::Invalid;

        mem::swap(&mut state, &mut self.action_state);

        match state {
            ActionPoll::Pending(ref mut poll) => {
                let out = poll.resolve_result(&self.runtime);

                if out.is_none() {
                    // Swap memory back
                    mem::swap(&mut state, &mut self.action_state);
                    return Err(QuicActionError::InProgress);
                }
            }
            ActionPoll::Completed(_) => todo!(),
            ActionPoll::Failed(error) => todo!(),
            ActionPoll::Invalid => todo!(),
        };

        todo!()
    }
}

#[derive(Clone, Debug)]
pub enum QuicActionError {
    InProgress,
    Consumed,
    Failed(ConnectionError),
    Crashed(Arc<JoinError>),
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
