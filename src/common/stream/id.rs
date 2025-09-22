use std::fmt::Display;

use bevy::ecs::component::Component;

use crate::common::IdGenerator;

#[derive(Debug, Default)]
pub(crate) struct StreamIdGenerator {
    generator: IdGenerator,
}

impl StreamIdGenerator {
    pub fn generate_id(&mut self) -> StreamId {
        StreamId {
            id: self.generator.generate_unique(),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Component, Clone, Copy)]
pub struct StreamId {
    id: u64,
}

impl Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamId({})", self.id)
    }
}
