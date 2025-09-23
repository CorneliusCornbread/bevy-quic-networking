use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bevy::{
    DefaultPlugins,
    app::{App, Startup},
    ecs::system::{Commands, Res},
    prelude::PluginGroup,
    render::{
        RenderPlugin,
        settings::{PowerPreference, WgpuSettings},
    },
};
use bevy_quic_networking::{
    QuicDefaultPlugins, client::QuicClient, common::connection::runtime::TokioRuntime,
    server::QuicServer,
};
use s2n_quic::{Client, Server, client::Builder};

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
        .add_plugins(QuicDefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

const IP: &str = "127.0.0.1:7777";

fn setup(mut commands: Commands, runtime: Res<TokioRuntime>) {
    let client_comp = QuicClient::new(&runtime);

    commands.spawn(client_comp);

    let ip: SocketAddr = IP.parse().unwrap();

    let server_comp = QuicServer::bind(&runtime, ip).expect("Unable to bind to server address");

    commands.spawn(server_comp);
}
