use bevy::ecs::{component::Component, world::FromWorld};
use tokio::runtime::Handle;

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
