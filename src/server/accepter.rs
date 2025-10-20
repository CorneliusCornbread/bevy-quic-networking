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
        app.add_systems(Update, accept_connections);
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
                info!("spawning connection");
            }
        }
    }
}

pub fn accept_streams(
    mut commands: Commands,
    query: Query<(Entity, &Children), With<QuicServer>>,
    tokio: Res<TokioRuntime>,
    mut connection_query: Query<(Entity, &mut QuicConnection)>,
) {
    let handle = tokio.handle();

    for (_server_entity, children) in query.iter() {
        for child in children {
            let conn_opt = connection_query.get_mut(*child).ok();
            if conn_opt.is_none() {
                continue;
            }

            let (conn_entity, mut connection) = conn_opt.unwrap();

            let res_stream = connection.accept_streams();

            if let Err(e) = res_stream {
                if let StreamPollError::Error(error) = e {
                    error!("Error polling for new stream: {:?}", error)
                }
                continue;
            }

            let (stream, id) = res_stream.unwrap();
            info!("spawning new stream");

            match stream {
                PeerStream::Bidirectional(bidirectional_stream) => {
                    let (rec, send) = bidirectional_stream.split();
                    let quic_rec = QuicReceiveStream::new(handle.clone(), rec);
                    let quic_send = QuicSendStream::new(handle.clone(), send);

                    let bundle = (quic_rec, quic_send, id, ChildOf(conn_entity));
                    commands.spawn(bundle);
                }
                PeerStream::Receive(receive_stream) => {
                    let quic_rec = QuicReceiveStream::new(handle.clone(), receive_stream);

                    let bundle = (quic_rec, id, ChildOf(conn_entity));
                    commands.spawn(bundle);
                }
            }
        }
    }
}
