//! # bevy-s2n-quic
//!
//! An Aeronet compatible Bevy plugin providing network IO with QUIC based on
//! [s2n-quic](https://github.com/aws/s2n-quic).
//!
//! ## Getting Started
//!
//! Add the default plugins to your app:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_s2n_quic::QuicDefaultPlugins;
//!
//! App::new()
//!     .add_plugins(QuicDefaultPlugins);
//! ```
//!
//! See the simple_net_system example for a basic setup of connecting a server and client
//!
//! ## Feature Flags
//!
//! | Flag | Description |
//! |------|-------------|
//! | `connection-errors` | Adds error components when connections fail |
//! | `stream-errors` | Adds error components when streams fail |
//! | `performance-warns` | Warns when buffers fill faster than they drain (default) |

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
