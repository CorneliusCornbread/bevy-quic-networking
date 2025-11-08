use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::{ChildOf, Children},
        query::With,
        system::{Commands, Query, Res},
    },
    log::{error, info},
};
use s2n_quic::stream::PeerStream;

use crate::{
    common::{
        connection::{QuicConnection, StreamPollError, runtime::TokioRuntime},
        stream::{receive::QuicReceiveStream, send::QuicSendStream},
    },
    server::QuicServer,
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
                let bundle = (quic_connection, connection_id, ChildOf(entity));
                commands.spawn(bundle);
            }
        }
    }
}

fn accept_streams(
    mut commands: Commands,
    mut connection_query: Query<(Entity, &mut QuicConnection, &ChildOf)>,
    server_query: Query<&QuicServer>,
    tokio: Res<TokioRuntime>,
) {
    let handle = tokio.handle();

    for (connection_entity, mut connection, parent) in connection_query.iter_mut() {
        if server_query.get(parent.parent()).is_ok()
            && let Ok((stream, id)) = connection.accept_streams()
        {
            match stream {
                PeerStream::Bidirectional(bidirectional_stream) => {
                    let (rec, send) = bidirectional_stream.split();
                    let quic_rec = QuicReceiveStream::new(handle.clone(), rec);
                    let quic_send = QuicSendStream::new(handle.clone(), send);

                    commands.entity(connection_entity).with_children(|parent| {
                        parent.spawn((quic_rec, quic_send, id));
                    });
                }
                PeerStream::Receive(receive_stream) => {
                    let quic_rec = QuicReceiveStream::new(handle.clone(), receive_stream);

                    commands.entity(connection_entity).with_children(|parent| {
                        parent.spawn((quic_rec, id));
                    });
                }
            }
        }
    }
}
