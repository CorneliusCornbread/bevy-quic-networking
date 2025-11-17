use bevy::{
    ecs::component::Component,
    prelude::{Deref, DerefMut},
};
use s2n_quic::Connection;
use tokio::runtime::Handle;

use crate::common::connection::QuicConnection;

#[derive(Debug, Component, Deref, DerefMut)]
pub struct QuicServerConnection {
    connection: QuicConnection,
}

impl QuicServerConnection {
    pub fn new(runtime: Handle, connection: Connection) -> Self {
        Self {
            connection: QuicConnection::new(runtime, connection),
        }
    }
}
