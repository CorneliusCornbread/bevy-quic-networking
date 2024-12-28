use bevy::{app::Plugin, ecs::schedule::ScheduleLabel};
use config::NetworkConfig;

pub mod config;
pub mod message;
pub mod transport;

pub const NETWORK_SCHEDULE_NAME: &str = "network_tick";

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NetworkUpdate;

pub struct TransportPlugin {
    use_default_transport: bool,
    config: NetworkConfig,
}

impl Plugin for TransportPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_schedule(NetworkUpdate);
        app.insert_resource(self.config.clone());

        if self.use_default_transport {
            app.add_systems(NetworkUpdate, transport::rec_messages);
        }
    }
}

impl Default for TransportPlugin {
    fn default() -> Self {
        Self {
            use_default_transport: true,
            config: Default::default(),
        }
    }
}

impl TransportPlugin {
    pub fn new(use_default_transport: bool, tick_rate: u16) -> Self {
        Self {
            use_default_transport,
            config: NetworkConfig::new(tick_rate),
        }
    }
}
