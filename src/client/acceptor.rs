use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res},
    },
};
use s2n_quic::stream::PeerStream;

use crate::{
    client::{
        connection::QuicClientConnection,
        marker::QuicClientMarker,
        stream::{QuicClientReceiveStream, QuicClientSendStream},
    },
    common::{connection::runtime::TokioRuntime, stream::session::QuicSession},
};

/// This plugin makes all clients accept all incoming streams and spawns them
/// as components parented to their [QuicClientConnection]s in the ECS world.
#[derive(Debug)]
pub struct SimpleClientAcceptorPlugin;

impl Plugin for SimpleClientAcceptorPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, accept_streams);
    }
}

fn accept_streams(
    mut commands: Commands,
    connection_query: Query<(Entity, &mut QuicClientConnection)>,
    tokio: Res<TokioRuntime>,
) {
    let handle = tokio.handle();

    for (connection_entity, mut connection) in connection_query {
        if let Ok((stream, id)) = connection.accept_streams() {
            match stream {
                PeerStream::Bidirectional(bidirectional_stream) => {
                    let (rec, send) = bidirectional_stream.split();
                    let quic_rec = QuicClientReceiveStream::new(handle.clone(), rec);
                    let quic_send = QuicClientSendStream::new(handle.clone(), send);

                    commands.entity(connection_entity).with_children(|parent| {
                        parent.spawn((quic_rec, quic_send, QuicClientMarker, QuicSession, id));
                    });
                }
                PeerStream::Receive(receive_stream) => {
                    let quic_rec = QuicClientReceiveStream::new(handle.clone(), receive_stream);

                    commands.entity(connection_entity).with_children(|parent| {
                        parent.spawn((quic_rec, QuicClientMarker, QuicSession, id));
                    });
                }
            }
        }
    }
}
