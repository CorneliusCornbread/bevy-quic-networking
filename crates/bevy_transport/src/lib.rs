use bevy::{
    app::{Plugin, Update},
    ecs::{
        resource::Resource,
        schedule::ScheduleLabel,
        world::{Mut, World},
    },
    log::warn,
    time::{Time, Timer},
};
use config::NetworkConfig;

pub mod config;
pub mod message;
pub mod transport;

pub const NETWORK_SCHEDULE_NAME: &str = "network_tick";

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NetworkUpdate;

#[derive(Resource)]
struct NetworkTimer(Timer);

pub struct TransportPlugin {
    use_default_transport: bool,
    config: NetworkConfig,
}

impl Plugin for TransportPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_schedule(NetworkUpdate);
        app.insert_resource(self.config.clone());
        app.insert_resource(NetworkTimer(Timer::from_seconds(
            1.0 / self.config.tick_rate() as f32,
            bevy::time::TimerMode::Repeating,
        )));

        if self.use_default_transport {
            app.add_systems(NetworkUpdate, transport::rec_messages);
        }

        app.add_systems(Update, net_schedule_runner);
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

pub fn net_schedule_runner(world: &mut World) {
    let time: &Time = world.get_resource().expect("No time resource available.");
    let delta = time.delta();
    let net_timer_opt: Option<Mut<NetworkTimer>> = world.get_resource_mut();

    if let Some(mut net_timer) = net_timer_opt {
        if net_timer.0.tick(delta).just_finished() {
            world.run_schedule(NetworkUpdate);
        }
    } else {
        warn!("Network schedule runner is enabled but there is no NetworkTimer resource.")
    }
}
