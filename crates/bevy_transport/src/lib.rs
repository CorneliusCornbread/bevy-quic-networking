use bevy::{app::Plugin, ecs::schedule::ScheduleLabel};

pub mod message;
pub mod transport;

pub const NETWORK_SCHEDULE_NAME: &str = "network_tick";

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NetworkUpdate;

pub struct TransportPlugin {
    use_default_transport: bool,
}

impl Plugin for TransportPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_schedule(NetworkUpdate);

        if self.use_default_transport {
            app.add_systems(NetworkUpdate, transport::rec_messages);
        }
    }
}
