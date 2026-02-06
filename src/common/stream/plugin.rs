use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        system::{Commands, Query},
    },
    log::{error, info, info_span},
};

use crate::{
    client::stream::{
        QuicClientBidirectionalStreamAttempt, QuicClientReceiveStream, QuicClientSendStream,
    },
    common::{
        attempt::QuicActionError,
        stream::{id::StreamId, session::QuicSession},
    },
    server::stream::{
        QuicServerBidirectionalStreamAttempt, QuicServerReceiveStream, QuicServerSendStream,
    },
};

#[derive(Debug)]
pub struct StreamAttemptPlugin;

impl Plugin for StreamAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, client_bidir_stream_attempt)
            .add_systems(Update, server_bidir_stream_attempt);
    }
}

fn client_bidir_stream_attempt(
    mut commands: Commands,
    query: Query<(Entity, &mut QuicClientBidirectionalStreamAttempt, &StreamId)>,
) {
    let _span = info_span!("client_bidir_stream_attempt").entered();

    for entity_bundle in query {
        let (entity, mut attempt, id) = entity_bundle;

        let res = attempt.attempt_result();

        if let Err(e) = res {
            match &e {
                QuicActionError::InProgress => continue,
                QuicActionError::Consumed => {
                    error!("Stream attempt consumed for entity: {:?}", entity)
                }
                QuicActionError::Failed(error) => {
                    error!("Stream attempt failed: {:?}", error)
                }
                QuicActionError::Crashed(join_error) => {
                    error!("Stream attempt crashed: {:?}", join_error)
                }
            }

            let mut error_entity = commands.entity(entity);

            #[cfg(feature = "stream-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id);

                error_entity.insert(err_bundle);
            }

            error_entity.remove::<QuicClientBidirectionalStreamAttempt>();

            continue;
        }

        let (rec, send) = res.unwrap();
        let send_stream = QuicClientSendStream::from_send_stream(send);
        let rec_stream = QuicClientReceiveStream::from_rec_stream(rec);

        info!("Spawning bidirectional stream with {id}");

        commands
            .entity(entity)
            .remove::<QuicClientBidirectionalStreamAttempt>()
            .insert((send_stream, rec_stream, QuicSession));
    }
}

fn server_bidir_stream_attempt(
    mut commands: Commands,
    query: Query<(Entity, &mut QuicServerBidirectionalStreamAttempt, &StreamId)>,
) {
    let _span = info_span!("server_bidir_stream_attempt").entered();

    for entity_bundle in query {
        let (entity, mut attempt, id) = entity_bundle;

        let res = attempt.get_output();

        if let Err(e) = res {
            match &e {
                QuicActionError::InProgress => continue,
                QuicActionError::Consumed => {
                    error!("Stream attempt consumed for entity: {:?}", entity)
                }
                QuicActionError::Failed(error) => {
                    error!("Stream attempt failed: {:?}", error)
                }
                QuicActionError::Crashed(join_error) => {
                    error!("Stream attempt crashed: {:?}", join_error)
                }
            }

            let mut error_entity = commands.entity(entity);

            #[cfg(feature = "stream-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id);

                error_entity.insert(err_bundle);
            }

            error_entity.remove::<QuicServerBidirectionalStreamAttempt>();

            continue;
        }

        let (rec, send) = res.unwrap();
        let send_stream = QuicServerSendStream::from_send_stream(send);
        let rec_stream = QuicServerReceiveStream::from_rec_stream(rec);

        info!("Spawning server bidirectional stream with {id}");

        commands
            .entity(entity)
            .remove::<QuicServerBidirectionalStreamAttempt>()
            .insert((send_stream, rec_stream, QuicSession));
    }
}
