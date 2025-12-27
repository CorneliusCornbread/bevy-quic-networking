use std::{net::SocketAddr, path::Path};

use bevy::{
    DefaultPlugins,
    app::{App, PostUpdate, Startup, Update},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        query::Without,
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res},
    },
    input::{common_conditions::input_just_pressed, keyboard::KeyCode},
    log::{error, info},
    prelude::PluginGroup,
    remote::{RemotePlugin, http::RemoteHttpPlugin},
    render::{
        RenderPlugin,
        settings::{PowerPreference, WgpuSettings},
    },
};
use bevy_quic_networking::{
    QuicDefaultPlugins,
    client::{
        QuicClient,
        connection::QuicClientConnection,
        stream::{QuicClientReceiveStream, QuicClientSendStream},
    },
    common::connection::runtime::TokioRuntime,
    server::{
        QuicServer,
        stream::{QuicServerReceiveStream, QuicServerSendStream},
    },
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
        .add_systems(PostUpdate, (client_open_stream, debug_receive, debug_send))
        .add_systems(
            PostUpdate,
            client_send.run_if(input_just_pressed(KeyCode::Space)),
        )
        .add_systems(
            PostUpdate,
            stop_receive.run_if(input_just_pressed(KeyCode::Digit0)),
        )
        .add_systems(
            PostUpdate,
            close_send.run_if(input_just_pressed(KeyCode::Digit9)),
        )
        .add_systems(Update, add_debug)
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
    let connect = Connect::new(ip).with_server_name("localhost");
    let conn_bundle = client_comp.open_connection(connect);

    // Spawn client with connection attempt as child
    commands.spawn(client_comp).with_children(|parent| {
        parent.spawn(conn_bundle);
    });
}

#[derive(Component)]
struct DebugCount(u64);

fn client_open_stream(
    mut commands: Commands,
    connection_query: Query<(Entity, &mut QuicClientConnection), Without<Children>>,
) {
    for (entity, mut connection) in connection_query {
        let stream_bundle = connection.open_bidrectional_stream();
        commands.spawn((stream_bundle, DebugCount(0), ChildOf(entity)));
    }
}

fn client_send(client_streams: Query<(&mut QuicClientSendStream, &mut DebugCount)>) {
    for (mut send_stream, mut debug_count) in client_streams {
        let res = send_stream.send("Yippieee".into());
        if let Err(e) = res {
            error!("Error sending data: {}", e);
            continue;
        }

        debug_count.0 += 1;

        info!("Message send count: {}", debug_count.0);
    }
}

fn add_debug(
    mut commands: Commands,
    query: Query<(Entity, &QuicServerReceiveStream), Without<DebugCount>>,
) {
    for (entity, _stream) in &query {
        commands.entity(entity).insert(DebugCount(0));
    }
}

fn debug_receive(
    server_receivers: Query<(&mut QuicServerReceiveStream, &mut DebugCount)>,
    client_receivers: Query<&mut QuicClientReceiveStream>,
) {
    for (mut stream, mut debug) in server_receivers {
        stream.log_outstanding_errors();

        if !stream.is_open() {
            continue;
        }

        if let Some(data) = stream.poll_recv() {
            let bytes = data.payload;
            let string = String::from_utf8_lossy(&bytes);
            info!("Received message: '{}'", string);
            debug.0 += 1;
            let count = debug.0;

            info!("Message rec count: {}", count);
        }
    }

    for mut stream in client_receivers {
        stream.log_outstanding_errors();
    }
}

fn debug_send(
    server_receivers: Query<&mut QuicServerSendStream>,
    client_receivers: Query<&mut QuicClientSendStream>,
) {
    for mut stream in server_receivers {
        stream.log_outstanding_errors();
    }

    for mut stream in client_receivers {
        stream.log_outstanding_errors();
    }
}

fn stop_receive(receivers: Query<&mut QuicServerReceiveStream>) {
    for mut stream in receivers {
        stream.stop_send(0u8.into());
    }
}

fn close_send(senders: Query<&mut QuicClientSendStream>) {
    for mut stream in senders {
        stream.close();
    }
}
