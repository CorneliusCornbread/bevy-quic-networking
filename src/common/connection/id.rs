use std::fmt;

use bevy::ecs::component::Component;

use crate::common::ConnectionType;

// TODO: make this not a component
#[derive(PartialEq, Eq, Debug, Component, Clone, Copy)]
pub struct ConnectionId {
    connection_type: ConnectionType,
    parent_id: u64,
    id: u64,
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConnectionId({})", self.id)
    }
}
