use std::{error::Error, sync::Arc};

use s2n_quic::Connection;

#[derive(Debug)]
pub enum ConnectionState {
    Connected(Connection),
    Failed(Box<dyn Error + Send>),
    InProgress,
}
