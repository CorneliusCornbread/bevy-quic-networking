pub mod async_plugin;
pub mod client;
pub mod common;
pub mod server;

use bevy::app::{PluginGroup, PluginGroupBuilder};

use crate::{
    async_plugin::QuicAsyncPlugin,
    client::acceptor::SimpleClientAcceptorPlugin,
    common::{
        connection::plugin::ConnectionAttemptPlugin,
        plugin::DisconnectHandlerPlugin,
        stream::{
            plugin::StreamAttemptPlugin,
            session::{QuicAeronetEventPlugin, QuicAeronetPacketPlugin},
        },
    },
    server::acceptor::SimpleServerAcceptorPlugin,
};

/// The default set of plugins needed to make the Bevy Quic components
/// function and handle errors and connection attempts.
pub struct QuicDefaultPlugins;

impl PluginGroup for QuicDefaultPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(QuicAsyncPlugin::default())
            .add(ConnectionAttemptPlugin)
            .add(StreamAttemptPlugin)
            .add(SimpleServerAcceptorPlugin)
            .add(SimpleClientAcceptorPlugin)
            .add(DisconnectHandlerPlugin)
    }
}

/// The plugin group for Aeronet event handling.
pub struct QuicAeronetPlugins;

impl PluginGroup for QuicAeronetPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(QuicAeronetPacketPlugin)
            .add(QuicAeronetEventPlugin)
    }
}
