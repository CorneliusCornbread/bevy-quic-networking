use bevy::ecs::component::Component;

use crate::common::IdGenerator;

#[derive(Default)]
pub struct ConnectionIdGenerator {
    generator: IdGenerator,
}

#[derive(PartialEq, Eq, Debug, Component, Clone, Copy)]
pub struct ConnectionId {
    id: u64,
}

impl ConnectionIdGenerator {
    pub fn generate_id(&mut self) -> ConnectionId {
        ConnectionId {
            id: self.generator.generate_unique(),
        }
    }
}
