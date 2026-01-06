#[cfg(feature = "aeronet_io")]
use aeronet_io::Session;

use bevy::{
    app::{Plugin, PreUpdate},
    ecs::{component::Component, query::With, system::Query},
    log::warn,
};
use std::time::Instant;

use crate::{
    client::stream::QuicClientSendStream, common::stream::id::StreamId,
    server::stream::QuicServerReceiveStream,
};

const MIN_MTU: usize = 1200;
const MAX_PACKET_TRANSFER: usize = 512;
const PACKET_WARN_THRESH: usize = 400;

#[derive(Component, Default)]
#[cfg_attr(feature = "aeronet_io", require(Session::new(Instant::now(), MIN_MTU)))]
pub struct QuicSession;

pub struct QuicAeronetPlugin;

impl Plugin for QuicAeronetPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(PreUpdate, drain_recv_packets)
            .add_systems(PreUpdate, drain_send_packets);
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
