pub mod client;
pub mod common;
pub mod plugin;
pub mod server;

use bevy::app::{PluginGroup, PluginGroupBuilder};
pub use s2n_quic::Server;

// Re-exports
pub use s2n_quic::client::Client;

use crate::{
    common::{
        connection::plugin::ConnectionAttemptPlugin,
        stream::{plugin::StreamAttemptPlugin, session::QuicSessionPacketPlugin},
    },
    plugin::QuicAsyncPlugin,
};

pub struct QuicDefaultPlugins;

impl PluginGroup for QuicDefaultPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(QuicAsyncPlugin::default())
            .add(ConnectionAttemptPlugin)
            .add(StreamAttemptPlugin)
            .add(QuicSessionPacketPlugin)
    }
}
