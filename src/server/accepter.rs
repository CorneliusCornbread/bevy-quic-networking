use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        system::{Commands, Query, Res},
    },
    log::error,
};
use s2n_quic::stream::PeerStream;

use crate::{
    common::{connection::runtime::TokioRuntime, stream::session::QuicSession},
    server::{
        QuicServer,
        connection::QuicServerConnection,
        marker::QuicServerMarker,
        stream::{QuicServerReceiveStream, QuicServerSendStream},
    },
};

#[derive(Debug)]
pub struct SimpleServerAccepterPlugin;

impl Plugin for SimpleServerAccepterPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, accept_connections)
            .add_systems(Update, accept_streams);
    }
}

pub fn accept_connections(mut commands: Commands, servers: Query<(&mut QuicServer, Entity)>) {
    for (mut server, entity) in servers {
        let res = server.poll_connection();

        if let Err(e) = res {
            error!("Error handling server connection: {}", e);
            continue;
        }

        let conn = res.unwrap();

        match conn {
            super::ConnectionPoll::None => continue,
            super::ConnectionPoll::ServerClosed => continue,
            super::ConnectionPoll::NewConnection(quic_connection, connection_id) => {
                let bundle = (
                    quic_connection,
                    connection_id,
                    QuicServerMarker,
                    ChildOf(entity),
                );
                commands.spawn(bundle);
            }
        }
    }
}

fn accept_streams(
    mut commands: Commands,
    connection_query: Query<(Entity, &mut QuicServerConnection)>,
    tokio: Res<TokioRuntime>,
) {
    let handle = tokio.handle();

    for (connection_entity, mut connection) in connection_query {
        if let Ok((stream, id)) = connection.accept_streams() {
            match stream {
                PeerStream::Bidirectional(bidirectional_stream) => {
                    let (rec, send) = bidirectional_stream.split();
                    let quic_rec = QuicServerReceiveStream::new(handle.clone(), rec);
                    let quic_send = QuicServerSendStream::new(handle.clone(), send);

                    commands.entity(connection_entity).with_children(|parent| {
                        parent.spawn((quic_rec, quic_send, QuicServerMarker, QuicSession, id));
                    });
                }
                PeerStream::Receive(receive_stream) => {
                    let quic_rec = QuicServerReceiveStream::new(handle.clone(), receive_stream);

                    commands.entity(connection_entity).with_children(|parent| {
                        parent.spawn((quic_rec, QuicServerMarker, QuicSession, id));
                    });
                }
            }
        }
    }
}
