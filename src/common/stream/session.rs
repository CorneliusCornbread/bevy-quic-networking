use aeronet_io::{Session, connection::Disconnected};
use bevy::{
    app::{Plugin, PostUpdate, PreUpdate},
    ecs::{
        entity::Entity,
        system::{Command, Commands},
    },
};
use std::time::Instant;

use crate::{
    client::stream::{QuicClientReceiveStream, QuicClientSendStream},
    common::stream::{disconnect::StreamDisconnectReason, id::StreamId},
    server::stream::{QuicServerReceiveStream, QuicServerSendStream},
};
use bevy::{
    ecs::{component::Component, query::With, system::Query},
    log::warn,
};

const MIN_MTU: usize = 1200;
const MAX_PACKET_TRANSFER: usize = 512;
const PACKET_WARN_THRESH: usize = 400;

#[derive(Component, Default)]
#[require(Session::new(Instant::now(), MIN_MTU))]
pub struct QuicSession;

// NOTE:
// Aeronet_IO is being left as a hard dependency as of now,
// the cost of having to convert to Aeronet structures is
// far too much of a maintenance burden due to unsafe requirements
// for efficient vec transmutation. We're using std::mem::swap
// to efficiency move buffers of data around and doing that
// efficiently when Aeronet is not a hard dep is a nightmare.
//
// Dropping Aeronet from your app is as simple as not including
// the Aeronet plugin in your app.
//
// The IO layer is a couple hundred lines of code, you may consume
// mine weiner if this is upsetting to you. - Cornelius
pub struct QuicAeronetPacketPlugin;

impl Plugin for QuicAeronetPacketPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(PreUpdate, (drain_recv_packets, drain_send_packets));
    }
}

pub struct QuicAeronetEventPlugin;

impl Plugin for QuicAeronetEventPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(
            PostUpdate,
            (
                fire_server_send_disconnect_events,
                fire_server_rec_disconnect_events,
                fire_client_send_disconnect_events,
                fire_client_rec_disconnect_events,
            ),
        );
    }
}

fn drain_recv_packets(
    query: Query<(&mut Session, &mut QuicServerReceiveStream), With<QuicSession>>,
) {
    let mut buffer = Vec::new();

    for entity in query {
        let (mut session, mut rec) = entity;

        let size = rec.blocking_recv_many(&mut buffer, MAX_PACKET_TRANSFER);

        if size >= PACKET_WARN_THRESH {
            #[cfg(feature = "performance-warns")]
            warn!(
                "Packet input is unexpectedly high. If the max transfer rate is exceeded this could cause delays in packet delivery. \nMax: {}\nCurrent: {}",
                MAX_PACKET_TRANSFER, size
            );
        }

        if session.recv.is_empty() {
            buffer.truncate(size);
            std::mem::swap(&mut buffer, &mut session.recv);
        } else {
            #[cfg(feature = "performance-warns")]
            warn!(
                "Session packet buffer is not empty. Are we falling behind on packet processing? Waiting packets: {}",
                session.recv.len()
            );

            session.recv.extend(buffer.drain(..size));
        }
    }
}

fn drain_send_packets(
    query: Query<(&mut Session, &mut QuicClientSendStream, &StreamId), With<QuicSession>>,
) {
    for entity in query {
        let (mut session, mut send, id) = entity;

        let res = send.send_many_drain(&mut session.send);

        if res.is_err() {
            #[cfg(feature = "performance-warns")]
            warn!(
                "Drain send unable to fully drain send buffer. Remaining items in buffer: {}\nHas the state of '{}' been corrupted?",
                session.send.len(),
                id
            )
        }
    }
}

fn fire_server_send_disconnect_events(
    mut cmd: Commands,
    query: Query<(&mut QuicServerReceiveStream, Entity), With<Session>>,
) {
    for (mut rec, entity) in query {
        if let Some(reason) = rec.get_disconnect_reason() {
            fire_disconnect(&mut cmd, entity, reason);
        }
    }
}

fn fire_server_rec_disconnect_events(
    mut cmd: Commands,
    query: Query<(&mut QuicServerSendStream, Entity), With<Session>>,
) {
    for (mut rec, entity) in query {
        if let Some(reason) = rec.get_disconnect_reason() {
            fire_disconnect(&mut cmd, entity, reason);
        }
    }
}

fn fire_client_send_disconnect_events(
    mut cmd: Commands,
    query: Query<(&mut QuicClientSendStream, Entity), With<Session>>,
) {
    for (mut rec, entity) in query {
        if let Some(reason) = rec.get_disconnect_reason() {
            fire_disconnect(&mut cmd, entity, reason);
        }
    }
}

fn fire_client_rec_disconnect_events(
    mut cmd: Commands,
    query: Query<(&mut QuicClientReceiveStream, Entity), With<Session>>,
) {
    for (mut rec, entity) in query {
        if let Some(reason) = rec.get_disconnect_reason() {
            fire_disconnect(&mut cmd, entity, reason);
        }
    }
}

fn fire_disconnect(cmd: &mut Commands, entity: Entity, reason: StreamDisconnectReason) {
    cmd.trigger(Disconnected {
        entity,
        reason: reason.into(),
    });
}
