use aeronet::io::Session;
use bevy::ecs::component::Component;
use std::time::Instant;

use crate::common::StreamId;

const MIN_MTU: usize = 1024;

#[derive(Component, Default)]
#[require(Session::new(Instant::now(), MIN_MTU))]
pub struct QuicSession;
