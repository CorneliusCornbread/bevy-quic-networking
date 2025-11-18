use bevy::ecs::{hierarchy::ChildOf, system::EntityCommands};
use s2n_quic::client::Connect;

use crate::client::QuicClient;

#[deprecated()]
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
