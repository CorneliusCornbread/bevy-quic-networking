use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        system::{Commands, Query},
    },
    log::{error, info},
};

use crate::common::{
    attempt::QuicActionError,
    stream::{QuicBidirectionalStreamAttempt, id::StreamId},
};

#[derive(Debug)]
pub struct StreamAttemptPlugin;

impl Plugin for StreamAttemptPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, handle_bidir_stream_attempt);
    }
}

fn handle_bidir_stream_attempt(
    mut commands: Commands,
    query: Query<(
        Entity,
        &mut QuicBidirectionalStreamAttempt,
        &StreamId,
        &ChildOf,
    )>,
) {
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

            #[cfg(feature = "stream-errors")]
            {
                use {crate::common::attempt::QuicActionErrorComponent, std::time::SystemTime};

                let err_comp = QuicActionErrorComponent::new(e, SystemTime::now());
                let err_bundle = (err_comp, *id, parent.clone());

                commands.spawn(err_bundle);
            }
            commands.entity(entity).despawn();

            continue;
        }

        let streams = res.unwrap();

        let bundle = (streams, *id, parent.clone());
        commands.entity(entity).despawn();
        commands.spawn(bundle);
        info!("Spawning stream")
    }
}
