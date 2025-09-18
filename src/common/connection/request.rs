use bevy::{
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        system::{Commands, EntityCommands, Query, Res},
        world::EntityRef,
    },
    log::error,
};
use s2n_quic::client::Connect;

use crate::{
    TokioRuntime,
    client::QuicClient,
    common::connection::{QuicConnection, QuicConnectionAttempt},
};

pub trait ConnectionRequestExt {
    fn request_client_connection(&mut self, client: &mut QuicClient, connect: Connect)
    -> &mut Self;
}

// TODO: streams should be sessions, not connections
impl<'a> ConnectionRequestExt for EntityCommands<'a> {
    fn request_client_connection(
        &mut self,
        client: &mut QuicClient,
        connect: Connect,
    ) -> &mut Self {
        let conn_bundle = client.open_connection(connect);
        let bundle = (conn_bundle.0, conn_bundle.1, ChildOf(self.id()));

        self.commands().spawn(bundle);
        self
    }
}

pub(crate) fn handle_connection_requests(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    query: Query<(Entity, &mut QuicConnectionAttempt)>,
) {
    let handle_ref = runtime.handle();

    for mut entity in query {
        let res = entity.1.get_output();

        if let Err(e) = res {
            error!("Error handling connection request: {:?}", e);
            continue;
        }

        let conn = res.unwrap();
        let quic_conn = QuicConnection::new(handle_ref.clone(), conn);

        commands.entity(entity.0).despawn();
    }
}
