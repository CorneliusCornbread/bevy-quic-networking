use aeronet::io::packet::RecvPacket;
use aeronet::io::Session;
use bevy::ecs::component::Component;
use bevy::prelude::Deref;
use s2n_quic::stream::SendStream;
use std::error::Error;

// TODO: Move connect, stream information, and data information into their own enums
#[derive(Debug)]
pub enum TransportData {
    Connected(StreamId),
    ConnectFailed(Box<dyn Error + Send>),
    ConnectInProgress,
    FailedStream(Box<dyn Error + Send>),
    ReceivedData(RecvPacket),
}

pub type QuicSession = QuicSessionInternal;

#[derive(Component)]
pub(crate) struct QuicSessionInternal(pub(crate) StreamId);

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
