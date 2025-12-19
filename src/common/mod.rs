use aeronet_io::packet::RecvPacket;
use bevy::{
    ecs::{entity::Entity, system::EntityCommands},
    log::{error, warn},
};
use std::{error::Error, sync::atomic::AtomicU64};
use tokio::sync::mpsc::error::TrySendError;

use crate::{
    client::marker::QuicClientMarker, common::stream::id::StreamId,
    server::marker::QuicServerMarker,
};

pub mod attempt;
pub mod connection;
pub mod status_code;
pub mod stream;

// TODO: Move connect, stream information, and data information into their own enums
#[derive(Debug)]
pub enum TransportData {
    Connected(StreamId),
    ConnectFailed(Box<dyn Error + Send>),
    ConnectInProgress,
    FailedStream(Box<dyn Error + Send>),
    ReceivedData(RecvPacket),
}

pub(crate) trait HandleChannelError {
    fn handle_err(&self);
}

impl<T> HandleChannelError for Result<(), TrySendError<T>> {
    fn handle_err(&self) {
        if let Err(send_err) = self {
            error!(
                "Error buffer for async task is full, the following error will be dropped: {send_err}"
            );
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct IdGenerator {
    current: AtomicU64,
}

impl IdGenerator {
    pub fn generate_unique(&mut self) -> u64 {
        self.current
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

pub fn handle_markers<'a>(
    e: &mut EntityCommands<'a>,
    entity: Entity,
    is_server: bool,
    is_client: bool,
) {
    match (is_client, is_server) {
        // client
        (true, false) => {
            e.insert(QuicClientMarker);
        }

        // server
        (false, true) => {
            e.insert(QuicServerMarker);
        }

        // both? weird state that shouldn't happen
        (true, true) => {
            warn!(
                "Entity {} had both client/server markers, this could result in weird behaviour.",
                entity
            );
            e.insert((QuicServerMarker, QuicClientMarker));
        }

        // neither? this is fine but it may be ignored by the different system queries
        (false, false) => {
            warn!(
                "Entity {} had no client/server markers, this may mean it's subsequent connection is not handled by systems which expect these markers.",
                entity
            );
        }
    };
}
