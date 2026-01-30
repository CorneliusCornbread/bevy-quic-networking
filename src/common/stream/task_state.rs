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
                self.task = Some(join);
                return None;
            }

            let join_res = self.runtime.block_on(join);

            if let Err(reason) = join_res {
                self.disconnect_reason =
                    Some(StreamDisconnectReason::InternalError(Arc::new(reason)));

                return self.disconnect_reason.clone();
            }

            self.disconnect_reason = Some(join_res.unwrap());

            return self.disconnect_reason.clone();
        }

        if self.disconnect_reason.is_none() {
            #[cfg(debug_assertions)]
            panic!(
                "Stream task is in invalid state, neither a join handle nor a disconnect reason was found."
            );

            #[cfg(not(debug_assertions))]
            bevy::log::error!(
                "Stream task is in invalid state, neither a join handle nor a disconnect reason was found. Returning none, this may result in weird behaviour."
            );
        }

        self.disconnect_reason.clone()
    }
}
