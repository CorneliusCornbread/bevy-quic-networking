use aeronet::io::packet::RecvPacket;
use bevy::log::error;
use std::error::Error;
use tokio::sync::mpsc::error::TrySendError;

use crate::common::stream::StreamId;

pub mod attempt;
pub mod connection;
pub mod status_code;
pub mod stream;

// TODO: Move connect, stream information, and data information into their own enums
#[derive(Debug)]
pub enum TransportData {
    Connected(StreamId),
    ConnectFailed(Box<dyn Error + Send>),
    ConnectInProgress,
    FailedStream(Box<dyn Error + Send>),
    ReceivedData(RecvPacket),
}

pub(crate) trait HandleChannelError {
    fn handle_err(&self);
}

impl<T> HandleChannelError for Result<(), TrySendError<T>> {
    fn handle_err(&self) {
        if let Err(send_err) = self {
            error!(
                "Error buffer for async task is full, the following error will be dropped: {send_err}"
            );
        }
    }
}
