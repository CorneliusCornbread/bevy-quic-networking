use bevy::{app::App, log::info, DefaultPlugins};
use bevy_transport::{NetworkUpdate, TransportPlugin};

fn main() {
    let _app = App::new()
        .add_plugins(DefaultPlugins)
        // Default will run 32 times per second with a simple transport system, for example's sake, we're updating once per second.
        .add_plugins(TransportPlugin::new(false, 1))
        .add_systems(NetworkUpdate, transport_update)
        .run();
}

fn transport_update() {
    info!("Transport update");
}
