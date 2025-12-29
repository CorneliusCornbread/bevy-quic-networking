use std::sync::Arc;

use tokio::{runtime::Handle, task::JoinHandle};

use crate::common::stream::disconnect::StreamDisconnectReason;

pub(in crate::common::stream) struct StreamTaskState {
    task: Option<JoinHandle<StreamDisconnectReason>>,
    disconnect_reason: Option<StreamDisconnectReason>,
    runtime: Handle,
}

impl StreamTaskState {
    pub fn new(runtime: Handle, task: JoinHandle<StreamDisconnectReason>) -> Self {
        Self {
            task: Some(task),
            disconnect_reason: None,
            runtime,
        }
    }

    pub fn is_finished(&self) -> bool {
        if let Some(join) = &self.task {
            return join.is_finished();
        }

        true
    }

    pub fn get_disconnect_reason(&mut self) -> Option<StreamDisconnectReason> {
        if let Some(join) = self.task.take() {
            if !join.is_finished() {
                return None;
            }

            let join_res = self.runtime.block_on(join);

            if let Err(reason) = join_res {
                return Some(StreamDisconnectReason::InternalError(Arc::new(reason)));
            }

            return Some(join_res.unwrap());
        }

        let reason = self.disconnect_reason
            .as_ref()
            .expect("Stream task is in invalid state, neither a join handle nor a disconnect reason was found.")
            .clone();

        Some(reason)
    }
}
