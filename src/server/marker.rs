use bevy::ecs::component::Component;

/// A marker component which can be used to uniquely identify any
/// [QuicConnection][crate::common::connection::QuicConnection]
/// as a [QuicServer][crate::server::QuicServer] created network resource.
#[derive(Component, Default)]
pub struct QuicServerMarker;
