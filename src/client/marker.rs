use bevy::ecs::component::Component;

/// A marker component which can be used to uniquely identify any
/// [QuicConnection][crate::common::connection::QuicConnection]
/// as a [QuicClient][crate::client::QuicClient] created network resource.
#[derive(Component, Default)]
pub struct QuicClientMarker;
