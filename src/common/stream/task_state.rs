use crate::common::{stream::disconnect::StreamDisconnectReason, task_state::QuicTaskState};

pub(in crate::common::stream) type StreamTaskState = QuicTaskState<StreamDisconnectReason>;
