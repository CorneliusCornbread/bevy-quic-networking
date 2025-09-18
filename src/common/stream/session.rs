use aeronet::io::Session;
use bevy::ecs::component::Component;
use std::time::Instant;

use crate::common::StreamId;

const MIN_MTU: usize = 1024;

#[derive(Component)]
#[require(Session::new(Instant::now(), MIN_MTU))]
pub struct QuicSession {
    id: StreamId,
}

impl QuicSession {
    pub(crate) fn new(id: StreamId) -> Self {
        Self { id }
    }

    pub fn stream_id(&self) -> StreamId {
        self.id
    }
}
