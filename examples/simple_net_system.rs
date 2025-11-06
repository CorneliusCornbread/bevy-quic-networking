use std::{net::SocketAddr, path::Path};

use bevy::{
    DefaultPlugins,
    app::{App, FixedUpdate, PostUpdate, Startup, Update},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        query::{With, Without},
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res},
    },
    input::{
        common_conditions::{input_just_pressed, input_pressed},
        keyboard::KeyCode,
    },
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
    client::QuicClient,
    common::{
        connection::{QuicConnection, request::ConnectionRequestExt, runtime::TokioRuntime},
        stream::{receive::QuicReceiveStream, request::StreamRequestExt, send::QuicSendStream},
    },
    server::QuicServer,
};
use s2n_quic::client::Connect;
use tokio::runtime;

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
            client_send.run_if(input_pressed(KeyCode::Space)),
        )
        .add_systems(Update, server_accept_streams)
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

fn client_open_stream(
    mut commands: Commands,
    mut connection_query: Query<(Entity, &mut QuicConnection, &ChildOf), Without<Children>>,
    client_query: Query<&QuicClient>,
) {
    for (connection_entity, mut connection, parent) in connection_query.iter_mut() {
        // TODO: I don't know why we need to call .entity here instead of an empty
        // Maybe we should rethink this API
        if client_query.get(parent.get()).is_ok() {
            commands
                .entity(connection_entity)
                .request_bidirectional_stream(&mut connection);
        }
    }
}

// Query for all streams under QuicClient connections
fn client_send(
    client_query: Query<&Children, With<QuicClient>>,
    connection_query: Query<&Children, With<QuicConnection>>,
    mut send_stream_query: Query<(Entity, &mut QuicSendStream)>,
    receive_stream_query: Query<(Entity, &QuicReceiveStream)>,
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

                        info!("data sent!");
                    }
                }
            }
        }
    }
}

fn debug_receive(receivers: Query<(&mut QuicReceiveStream, Entity)>) {
    for (mut stream, entity) in receivers {
        if !stream.is_open() {
            error_once!("Stream closed");
        }

        stream.print_rec_errors();

        if let Some(data) = stream.poll_recv() {
            let bytes = data.payload;
            let string = String::from_utf8_lossy(&bytes);
            info_once!("Received message:\n{}", string);
        }
    }
}

fn server_accept_streams(
    mut commands: Commands,
    mut connection_query: Query<(Entity, &mut QuicConnection, &ChildOf)>,
    server_query: Query<&QuicServer>,
    tokio: Res<TokioRuntime>,
) {
    let runtime = tokio.handle();

    for (connection_entity, mut connection, parent) in connection_query.iter_mut() {
        // Only proceed if this connection's parent is a QuicServer
        if server_query.get(parent.get()).is_ok() {
            // Accept any pending streams
            // (Adjust this based on your actual QuicConnection API)
            if let Ok((stream, id)) = connection.accept_streams() {
                // Spawn the stream as a child of this connection
                match stream {
                    s2n_quic::stream::PeerStream::Bidirectional(bidirectional_stream) => {
                        let (rec, send) = bidirectional_stream.split();
                        let send_comp = QuicSendStream::new(runtime.clone(), send);
                        let rec_comp = QuicReceiveStream::new(runtime.clone(), rec);

                        commands.entity(connection_entity).with_children(|parent| {
                            parent.spawn((send_comp, rec_comp));
                        });
                    }
                    s2n_quic::stream::PeerStream::Receive(receive_stream) => todo!(),
                }
            }
        }
    }
}
