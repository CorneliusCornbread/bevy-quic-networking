use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res},
    },
    log::{error, info, info_span},
};

use crate::{
    client::connection::{QuicClientConnection, QuicClientConnectionAttempt},
    common::{
        attempt::QuicActionError,
        connection::{QuicConnection, id::ConnectionId, runtime::TokioRuntime},
    },
    server::connection::{QuicServerConnection, QuicServerConnectionAttempt},
};

#[derive(Debug)]
pub struct ConnectionAttemptPlugin;

impl Plugin for ConnectionAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, server_connection_attempt)
            .add_systems(Update, client_connection_attempt);
    }
}

fn server_connection_attempt(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    query: Query<(Entity, &mut QuicServerConnectionAttempt, &ConnectionId)>,
) {
    let _span = info_span!("server_connection_attempt").entered();

    let handle_ref = runtime.handle();

    for entity_bundle in query {
        let (entity, mut attempt, id) = entity_bundle;

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

            let mut error_entity = commands.entity(entity);

            #[cfg(feature = "connection-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id);

                error_entity
                    .remove::<QuicServerConnectionAttempt>()
                    .insert(err_bundle);
            }

            error_entity.remove::<QuicServerConnectionAttempt>();

            continue;
        }

        info!("Spawning server connection entity with {id}");
        let conn = res.unwrap();
        let quic_conn = QuicConnection::new(handle_ref.clone(), conn);
        let conn_bundle = QuicServerConnection::from_connection(quic_conn);

        commands
            .entity(entity)
            .remove::<QuicServerConnectionAttempt>()
            .insert(conn_bundle);
    }
}

fn client_connection_attempt(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    query: Query<(Entity, &mut QuicClientConnectionAttempt, &ConnectionId)>,
) {
    let _span = info_span!("client_connection_attempt").entered();

    let handle_ref = runtime.handle();

    for entity_bundle in query {
        let (entity, mut attempt, id) = entity_bundle;

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

            let mut error_entity = commands.entity(entity);

            //#[cfg(feature = "connection-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id);

                error_entity
                    .remove::<QuicClientConnectionAttempt>()
                    .insert(err_bundle);
            }

            error_entity.remove::<QuicClientConnectionAttempt>();

            continue;
        }

        info!("Spawning client connection entity with {id}");
        let conn = res.unwrap();
        let quic_conn = QuicConnection::new(handle_ref.clone(), conn);
        let conn_bundle = QuicClientConnection::from_connection(quic_conn);

        commands
            .entity(entity)
            .remove::<QuicClientConnectionAttempt>()
            .insert(conn_bundle);
    }
}
