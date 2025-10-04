use std::{net::SocketAddr, path::Path};

use bevy::{
    DefaultPlugins,
    app::{App, PostUpdate, Startup},
    ecs::{
        entity::Entity,
        hierarchy::Children,
        query::{With, Without},
        system::{Commands, Query, Res},
    },
    log::{info, info_once},
    prelude::PluginGroup,
    remote::{RemotePlugin, http::RemoteHttpPlugin},
    render::{
        RenderPlugin,
        settings::{PowerPreference, WgpuSettings},
    },
};
use bevy_quic_networking::{
    QuicDefaultPlugins,
    client::QuicClient,
    common::{
        connection::{QuicConnection, request::ConnectionRequestExt, runtime::TokioRuntime},
        stream::request::StreamRequestExt,
    },
    server::QuicServer,
};
use s2n_quic::client::Connect;

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
        .add_plugins(RemotePlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, debug_server)
        //        .add_systems(PostUpdate, open_stream_client)
        .run();
}

const IP: &str = "127.0.0.1:7777";

fn setup(mut commands: Commands, runtime: Res<TokioRuntime>) {
    let cert_str_path = format!("{}/examples/certs/cert.pem", env!("CARGO_MANIFEST_DIR"));
    let cert_path = Path::new(&cert_str_path);

    let key_str_path = format!("{}/examples/certs/key.pem", env!("CARGO_MANIFEST_DIR"));
    let key_path = Path::new(&key_str_path);

    let ip: SocketAddr = IP.parse().unwrap();
    info!("IP set to: {}", ip);
    let server_comp = QuicServer::bind(&runtime, ip, cert_path, key_path)
        .expect("Unable to bind to server address");

    commands.spawn(server_comp);

    let mut client_comp = QuicClient::new_with_tls(&runtime, cert_path).expect("Invalid cert");

    commands
        .spawn_empty()
        .request_client_connection(
            &mut client_comp,
            Connect::new(ip).with_server_name("localhost"),
        )
        .insert(client_comp);
}

fn open_stream_client(
    mut commands: Commands,
    quic_clients: Query<(Entity, &QuicClient, &Children)>,
    mut quic_connections: Query<(Entity, &mut QuicConnection), Without<Children>>,
) {
    for (client_entity, client, children) in quic_clients.iter() {
        // Get all children that have QuicConnection but no Children component
        for &child in children.iter() {
            if let Ok((child_entity, mut connection)) = quic_connections.get_mut(child) {
                info!(
                    "QuicClient {:?} has no streams for connection {:?}",
                    client_entity, child_entity
                );

                commands
                    .spawn_empty()
                    .request_bidirectional_stream(&mut connection);
            }
        }
    }
}

fn debug_server(servers: Query<&mut QuicServer>) {
    for mut server in servers {
        let res = server.poll_connection();

        if res.is_err() {
            info!("poll error");
            continue;
        }

        let conn = res.unwrap();

        match conn {
            bevy_quic_networking::server::ConnectionPoll::None => (),
            bevy_quic_networking::server::ConnectionPoll::ServerClosed => {
                info_once!("server closed")
            }
            // TODO: add connection handling for server to spawn connection component
            bevy_quic_networking::server::ConnectionPoll::NewConnection(
                quic_connection,
                connection_id,
            ) => info!("server new connection"),
        }
    }
}
