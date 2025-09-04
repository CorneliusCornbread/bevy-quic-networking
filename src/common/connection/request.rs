use bevy::ecs::{hierarchy::ChildOf, system::EntityCommands};
use s2n_quic::client::Connect;

use crate::{client::QuicClient, common::connection::QuicConnection};

pub trait ConnectionRequestExt {
    fn request_client_connection(&mut self, client: &mut QuicClient, connect: Connect)
    -> &mut Self;
}

impl<'a> ConnectionRequestExt for EntityCommands<'a> {
    fn request_client_connection(
        &mut self,
        client: &mut QuicClient,
        connect: Connect,
    ) -> &mut Self {
        let conn_bundle = client.open_connection(connect);
        let bundle = (conn_bundle.0, conn_bundle.1, ChildOf(self.id()));

        self.commands().spawn(bundle);
        self
    }
}

pub trait StreamRequestExt {
    fn request_bidirectional_stream(&mut self, connection: &mut QuicConnection) -> &mut Self;
    fn request_receive_stream(&mut self, connection: &mut QuicConnection) -> &mut Self;
}

// TODO: streams should be sessions, not connections
impl<'a> StreamRequestExt for EntityCommands<'a> {
    fn request_bidirectional_stream(&mut self, connection: &mut QuicConnection) -> &mut Self {
        let stream_attempt = connection.open_bidrectional_stream();
        let bundle = (stream_attempt, ChildOf(self.id()));

        self.commands().spawn(bundle);
        self
    }

    fn request_receive_stream(&mut self, connection: &mut QuicConnection) -> &mut Self {
        todo!()
    }
}
