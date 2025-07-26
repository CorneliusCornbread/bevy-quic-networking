use aeronet::io::Session;
use aeronet::io::packet::RecvPacket;
use bevy::ecs::component::Component;
use bevy::ecs::world::FromWorld;
use bevy::prelude::Deref;
use s2n_quic::stream::SendStream;
use std::error::Error;
use tokio::runtime::Handle;

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

use crate::TokioRuntime;

#[derive(Component)]
pub(crate) struct ConnectionHandler {
    runtime: Handle,
}

impl FromWorld for ConnectionHandler {
    fn from_world(world: &mut bevy::ecs::world::World) -> Self {
        let runtime = world
            .get_resource_or_init::<TokioRuntime>()
            .handle()
            .clone();

        ConnectionHandler { runtime }
    }
}
