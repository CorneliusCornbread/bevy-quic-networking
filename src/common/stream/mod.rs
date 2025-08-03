use bevy::prelude::Deref;
use s2n_quic::stream::SendStream;

use crate::common::{
    attempt::QuicActionAttempt,
    stream::{receive::QuicReceiveStream, send::QuicSendStream},
};

pub mod receive;
pub mod send;
pub mod session;

pub type QuicReceiveStreamAttempt = QuicActionAttempt<QuicReceiveStream, StreamId>;
pub type QuicSendStreamAttempt = QuicActionAttempt<QuicReceiveStream, StreamId>;
pub type QuicBidirectionalStreamAttempt =
    QuicActionAttempt<(QuicReceiveStream, QuicSendStream), StreamId>;

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
