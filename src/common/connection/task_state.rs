use crate::common::{
    connection::disconnect::ConnectionDisconnectReason, task_state::QuicTaskState,
};

pub(in crate::common::connection) type ConnectionTaskState =
    QuicTaskState<ConnectionDisconnectReason>;
