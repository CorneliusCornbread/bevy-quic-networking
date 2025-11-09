use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        query::Has,
        system::{Commands, Query, Res},
    },
    log::{error, info, info_span},
};

use crate::{
    client::marker::QuicClientMarker,
    common::{
        attempt::QuicActionError,
        connection::{
            QuicConnection, QuicConnectionAttempt, id::ConnectionId, runtime::TokioRuntime,
        },
        handle_markers,
    },
    server::marker::QuicServerMarker,
};

#[derive(Debug)]
pub struct ConnectionAttemptPlugin;

impl Plugin for ConnectionAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, handle_connection_attempt);
    }
}
// Queries are going to be complex, wrapping them in a type is going to make
// the system query harder to read
#[allow(clippy::type_complexity)]
fn handle_connection_attempt(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    query: Query<(
        Entity,
        &mut QuicConnectionAttempt,
        &ConnectionId,
        &ChildOf,
        Has<QuicClientMarker>,
        Has<QuicServerMarker>,
    )>,
) {
    let _span = info_span!("handle_connection_attempt").entered();

    let handle_ref = runtime.handle();

    for entity_bundle in query {
        let (entity, mut attempt, id, parent, client_marker, server_marker) = entity_bundle;

        let res = attempt.get_output();

        if let Err(e) = res {
            match e {
                QuicActionError::InProgress => {
                    continue;
                } // TODO: Setup a timeout
                QuicActionError::Consumed => {
                    info!("Already consumed connection attempt hasn't been cleaned up: {entity}");
                }
                QuicActionError::Failed(error) => {
                    error!("Error handling connection attempt: {:?}", error)
                }
                QuicActionError::Crashed(ref join_error) => {
                    error!("Error joining connection attempt: {:?}", join_error)
                }
            }

            #[cfg(feature = "connection-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id, parent.clone());

                commands.spawn(err_bundle);
            }

            commands.entity(entity).despawn();

            continue;
        }

        info!("Spawning connection entity with {id}");
        let conn = res.unwrap();
        let quic_conn = QuicConnection::new(handle_ref.clone(), conn);

        let bundle = (quic_conn, *id, parent.clone());
        commands.entity(entity).despawn();
        let mut e = commands.spawn(bundle);

        handle_markers(&mut e, entity, server_marker, client_marker);
    }
}
