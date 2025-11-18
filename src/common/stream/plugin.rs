use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
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
};

#[derive(Debug)]
pub struct StreamAttemptPlugin;

impl Plugin for StreamAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, client_bidir_stream_attempt);
    }
}

fn client_bidir_stream_attempt(
    mut commands: Commands,
    query: Query<(
        Entity,
        &mut QuicClientBidirectionalStreamAttempt,
        &StreamId,
        &ChildOf,
    )>,
) {
    let _span = info_span!("handle_bidir_stream_attempt").entered();

    for entity_bundle in query {
        let (entity, mut attempt, id, parent) = entity_bundle;

        let res = attempt.get_output();

        if let Err(e) = res {
            match e {
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
                let err_bundle = (err_comp, *id, parent.clone());

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
