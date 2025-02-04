use bevy::{
    app::App,
    log::info,
    prelude::PluginGroup,
    render::{
        settings::{PowerPreference, WgpuSettings},
        RenderPlugin,
    },
    DefaultPlugins,
};
use bevy_transport::{NetworkUpdate, TransportPlugin};

fn main() {
    let _app = App::new()
        .add_plugins(
            // Set the default render settings to low power,
            // works nicer on dGPU systems with the dGPU disabled.
            // We're not rendering anything anyways.
            DefaultPlugins.set(RenderPlugin {
                render_creation: WgpuSettings {
                    power_preference: PowerPreference::LowPower,
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            }),
        )
        // Default will run 32 times per second with a simple transport system, for example's sake, we're updating once per second.
        .add_plugins(TransportPlugin::new(false, 1))
        .add_systems(NetworkUpdate, transport_update)
        .run();
}

fn transport_update() {
    info!("Transport update");
}
