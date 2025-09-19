use std::time::SystemTime;

use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        resource::Resource,
        system::{Commands, Query, Res},
    },
    log::error,
};

use crate::common::{
    attempt::QuicActionErrorComponent,
    connection::{QuicConnection, QuicConnectionAttempt, id::ConnectionId, runtime::TokioRuntime},
};

#[derive(Default, Debug, Resource)]
struct ConnAttemptConfig {
    pub(crate) error_entities: bool,
}

#[derive(Default, Debug)]
pub struct ConnectionAttemptPlugin {
    spawn_error_entities: bool,
}

impl Plugin for ConnectionAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, handle_connection_attempt);
        app.insert_resource(ConnAttemptConfig {
            error_entities: self.spawn_error_entities,
        });
    }
}

fn handle_connection_attempt(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    config: Res<ConnAttemptConfig>,
    query: Query<(Entity, &mut QuicConnectionAttempt, &ConnectionId, &ChildOf)>,
) {
    let handle_ref = runtime.handle();

    for entity_bundle in query {
        let (entity, mut attempt, id, parent) = entity_bundle;

        let res = attempt.get_output();

        if let Err(e) = res {
            error!("Error handling connection request: {:?}", e);

            if config.error_entities {
                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id);

                commands.spawn(err_bundle);
            }
            commands.entity(entity).despawn();

            continue;
        }

        let conn = res.unwrap();
        let quic_conn = QuicConnection::new(handle_ref.clone(), conn);

        let bundle = (quic_conn, *id, parent.clone());
        commands.entity(entity).despawn();
        commands.spawn(bundle);
    }
}
