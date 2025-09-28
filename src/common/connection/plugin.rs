use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        system::{Commands, Query, Res},
    },
    log::error,
};

use crate::common::{
    attempt::QuicActionError,
    connection::{QuicConnection, QuicConnectionAttempt, id::ConnectionId, runtime::TokioRuntime},
};

#[derive(Debug)]
pub struct ConnectionAttemptPlugin;

impl Plugin for ConnectionAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, handle_connection_attempt);
    }
}

fn handle_connection_attempt(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    query: Query<(Entity, &mut QuicConnectionAttempt, &ConnectionId, &ChildOf)>,
) {
    let handle_ref = runtime.handle();

    for entity_bundle in query {
        let (entity, mut attempt, id, parent) = entity_bundle;

        let res = attempt.get_output();

        if let Err(e) = res {
            match e {
                QuicActionError::InProgress => {
                    bevy::log::info_once!("In progress");

                    continue;
                } // TODO: Setup a timeout
                QuicActionError::Consumed => {
                    bevy::log::info_once!("Consumed");

                    continue; // Ignore dead components that 
                }
                QuicActionError::Failed(error) => {
                    bevy::log::error!("Error handling connection attempt: {:?}", error)
                }
                QuicActionError::Crashed(ref join_error) => {
                    bevy::log::error!("Error handling connection attempt: {:?}", join_error)
                }
            }

            #[cfg(feature = "connection-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id, parent.clone());

                commands.spawn(err_bundle);
            }

            bevy::log::info!("despawning attempt entity");
            commands.entity(entity).despawn();

            continue;
        }

        bevy::log::info!("Spawning connection entity");
        let conn = res.unwrap();
        let quic_conn = QuicConnection::new(handle_ref.clone(), conn);

        let bundle = (quic_conn, *id, parent.clone());
        commands.entity(entity).despawn();
        commands.spawn(bundle);
    }
}
