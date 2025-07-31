use aeronet::io::packet::RecvPacket;
use bevy::{log::error, prelude::Deref};
use s2n_quic::stream::SendStream;
use std::error::Error;
use tokio::sync::mpsc::error::TrySendError;

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

#[derive(Deref, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct StreamId(u64);

impl From<&SendStream> for StreamId {
    fn from(value: &SendStream) -> Self {
        value.stream_id()
    }
}

impl From<u64> for StreamId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

pub trait IntoStreamId {
    fn stream_id(&self) -> StreamId;
}

impl IntoStreamId for SendStream {
    fn stream_id(&self) -> StreamId {
        StreamId(self.id())
    }
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
