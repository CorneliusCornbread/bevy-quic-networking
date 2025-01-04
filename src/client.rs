use std::net::SocketAddr;

use bevy::prelude::Resource;

#[derive(Resource)]
pub struct QuicClient {
    runtime: tokio::runtime::Handle,
    connections: Vec<SocketAddr>,
}
