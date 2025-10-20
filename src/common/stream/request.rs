use bevy::ecs::{component::Component, hierarchy::ChildOf, system::EntityCommands};

use crate::common::connection::QuicConnection;

pub trait StreamRequestExt {
    fn request_bidirectional_stream(&mut self, connection: &mut QuicConnection) -> &mut Self;
    fn request_receive_stream(&mut self, connection: &mut QuicConnection) -> &mut Self;
}

impl<'a> StreamRequestExt for EntityCommands<'a> {
    fn request_bidirectional_stream(&mut self, connection: &mut QuicConnection) -> &mut Self {
        let stream_bundle = connection.open_bidrectional_stream();
        let bundle = (stream_bundle.0, stream_bundle.1, ChildOf(self.id()));

        self.commands().spawn(bundle);
        self
    }

    fn request_receive_stream(&mut self, connection: &mut QuicConnection) -> &mut Self {
        todo!()
    }
}
