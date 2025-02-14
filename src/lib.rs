use bevy::{
    app::Plugin,
    log::info,
    prelude::{Deref, DerefMut, Resource},
};
use bevy_transport::{config::NetworkConfig, TransportPlugin};
use tokio::runtime::Runtime;

pub mod common;
pub mod server;

pub struct QuicPlugin {
    tick_rate: u16,
}

impl Plugin for QuicPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        // NOTE: If bevy plugin dependencies ever get added this may not be necessary
        if !app.is_plugin_added::<TransportPlugin>() {
            app.add_plugins(TransportPlugin::new(true, self.tick_rate));
        } else {
            info!("Transport plugin was already initialized. Make sure the system for NetworkUpdate is handled, either by the default transport or your own.");
            app.insert_resource(NetworkConfig::new(self.tick_rate));
            app.init_resource::<TokioRuntime>();
        }
    }
}

impl Default for QuicPlugin {
    fn default() -> Self {
        Self {
            tick_rate: bevy_transport::config::DEFAULT_TICK_RATE,
        }
    }
}

impl QuicPlugin {
    pub fn tick_rate(&self) -> u16 {
        self.tick_rate
    }

    pub fn new(tick_rate: u16) -> Self {
        Self { tick_rate }
    }
}

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct TokioRuntime(pub(crate) Runtime);

impl Default for TokioRuntime {
    fn default() -> Self {
        Self(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Unable to create async runtime."),
        )
    }
}
