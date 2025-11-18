use std::{net::SocketAddr, path::Path};

use bevy::{
    DefaultPlugins,
    app::{App, PostUpdate, Startup},
    ecs::{
        entity::Entity,
        hierarchy::{ChildOf, Children},
        query::{With, Without},
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res},
    },
    input::{common_conditions::input_just_pressed, keyboard::KeyCode},
    log::{error, error_once, info, info_once},
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
        QuicClient, connection::QuicClientConnection, marker::QuicClientMarker,
        stream::QuicClientSendStream,
    },
    common::{
        connection::runtime::TokioRuntime,
        stream::{receive::QuicReceiveStream, send::QuicSendStream},
    },
    server::{QuicServer, stream::QuicServerReceiveStream},
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
        .add_systems(PostUpdate, client_open_stream)
        .add_systems(PostUpdate, debug_receive)
        .add_systems(
            PostUpdate,
            client_send.run_if(input_just_pressed(KeyCode::Space)),
        )
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

fn client_open_stream(
    mut commands: Commands,
    connection_query: Query<(Entity, &mut QuicClientConnection), Without<Children>>,
) {
    for (entity, mut connection) in connection_query {
        let stream_bundle = connection.open_bidrectional_stream();
        commands.spawn((stream_bundle, ChildOf(entity)));
    }
}

// Query for all streams under QuicClient connections
fn client_send(
    client_query: Query<&Children, With<QuicClient>>,
    connection_query: Query<&Children, With<QuicClientConnection>>,
    mut send_stream_query: Query<(Entity, &mut QuicClientSendStream)>,
) {
    for client_children in client_query.iter() {
        for &connection_entity in client_children.iter() {
            // Check if this child is a QuicConnection
            if let Ok(connection_children) = connection_query.get(connection_entity) {
                // Iterate through the connection's children to find streams
                for &stream_entity in connection_children.iter() {
                    if let Ok((entity, mut send_stream)) = send_stream_query.get_mut(stream_entity)
                    {
                        info_once!("Found client QuicSendStream {:?}", entity);
                        let res = send_stream.send("Yippieee".into());
                        if let Err(e) = res {
                            error!("Error sending data: {}", e);
                        }
                    }
                }
            }
        }
    }
}

fn client_send_easy(client_streams: Query<(&QuicClientSendStream)>) {
    for (send_stream) in client_streams {}
}

fn debug_receive(receivers: Query<(&mut QuicServerReceiveStream, Entity)>) {
    for (mut stream, entity) in receivers {
        if !stream.is_open() {
            error_once!("Stream closed");
        }

        stream.print_rec_errors();

        if let Some(data) = stream.poll_recv() {
            let bytes = data.payload;
            let string = String::from_utf8_lossy(&bytes);
            info!("Received message: '{}'", string);
        }
    }
}
