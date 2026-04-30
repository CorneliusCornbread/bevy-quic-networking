use aeronet_io::packet::RecvPacket;
use bevy::{
    ecs::{entity::Entity, system::EntityCommands},
    log::{error, warn},
};
use std::{error::Error, fmt};
use tokio::sync::mpsc::error::TrySendError;

use crate::{
    client::marker::QuicClientMarker,
    common::{id::IdGenerator, stream::id::StreamId},
    server::marker::QuicServerMarker,
};

pub mod attempt;
pub mod connection;
pub(crate) mod id;
pub mod runtime;
pub mod status_code;
pub mod stream;
pub(crate) mod task_state;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum QuicParentType {
    Server,
    Client,
}

impl fmt::Display for QuicParentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuicParentType::Client => write!(f, "Client"),
            QuicParentType::Server => write!(f, "Server"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct QuicParentId {
    parent_type: QuicParentType,
    parent_id: u64,
}

impl QuicParentId {
    pub fn new(parent_id: u64, parent_type: QuicParentType) -> Self {
        Self {
            parent_type,
            parent_id,
        }
    }

    pub fn generate_unique(parent_type: QuicParentType) -> Self {
        Self {
            parent_type,
            parent_id: IdGenerator::generate_unique(),
        }
    }

    pub fn parent_id(&self) -> u64 {
        self.parent_id
    }

    pub fn connection_type(&self) -> QuicParentType {
        self.parent_type
    }
}

impl fmt::Display for QuicParentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Id: {}", self.parent_type, self.parent_id)
    }
}

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
                "Error buffer for async task is full or closed, the following error will be dropped: {send_err}"
            );
        }
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
