use bevy::ecs::component::Component;

use crate::common::IdGenerator;

#[derive(Debug, Default)]
pub(crate) struct StreamIdGenerator {
    generator: IdGenerator,
}

#[derive(PartialEq, Eq, Debug, Component, Clone, Copy)]
pub struct StreamId {
    id: u64,
}

impl StreamIdGenerator {
    pub fn generate_id(&mut self) -> StreamId {
        StreamId {
            id: self.generator.generate_unique(),
        }
    }
}
