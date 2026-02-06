use std::{error::Error, net::SocketAddr, sync::Arc};

use bevy::ecs::component::Component;
use s2n_quic::Server;
use s2n_quic_tls::certificate::{IntoCertificate, IntoPrivateKey};
use tokio::{runtime::Handle, sync::Mutex, task::JoinError};

use crate::{
    common::connection::{
        id::{ConnectionId, ConnectionIdGenerator},
        runtime::TokioRuntime,
    },
    server::{connection::QuicServerConnection, marker::QuicServerMarker},
};

pub mod acceptor;
pub mod connection;
pub mod marker;
pub mod stream;

/// The component which manages an instance of a QuicServer.
///
/// It is recommended you parent any [QuicServerConnection] to their related QuicServer entity.
#[derive(Component)]
#[require(QuicServerMarker)]
pub struct QuicServer {
    runtime: Handle,
    server: Arc<Mutex<Server>>,
    id_gen: ConnectionIdGenerator,
}

impl QuicServer {
    /// Creates a new QuicServer and binds it to the given address with the given certificates.
    ///
    /// QUIC requires some form of TLS certificate, this function accepts the same kinds of certs the regular s2n-quic
    /// `bind()` function does.
    pub fn bind<C: IntoCertificate, PK: IntoPrivateKey>(
        runtime: &TokioRuntime,
        bind_ip: SocketAddr,
        certificate: C,
        private_key: PK,
    ) -> Result<Self, Box<dyn Error>> {
        let handle = runtime.handle().clone();
        let server = runtime.block_on(build_server(bind_ip, certificate, private_key))?;

        let server_mutex = Arc::new(Mutex::new(server));

        Ok(Self {
            runtime: handle,
            server: server_mutex,
            id_gen: Default::default(),
        })
    }

    /// Polls to receive any new pending connections from the async thread.
    ///
    /// The async thread will automatically accept any pending connections.
    /// It will then put them in a queue which is processed by this function.
    pub fn accept_connection(&mut self) -> Result<ConnectionPoll, JoinError> {
        let waker = Arc::new(futures::task::noop_waker_ref());
        let mut cx = std::task::Context::from_waker(&waker);

        let mut lock = self.server.blocking_lock();
        let poll = lock.poll_accept(&mut cx);
        drop(lock);

        match poll {
            std::task::Poll::Ready(conn_opt) => {
                if let Some(conn) = conn_opt {
                    let ret = ConnectionPoll::NewConnection(
                        QuicServerConnection::new(self.runtime.clone(), conn),
                        self.id_gen.generate_id(),
                    );

                    Ok(ret)
                } else {
                    bevy::log::info!(
                        "Server connection poll returned none, is our server not running?"
                    );
                    Ok(ConnectionPoll::ServerClosed)
                }
            }
            std::task::Poll::Pending => Ok(ConnectionPoll::None),
        }
    }
}

#[derive(Debug)]
pub enum ConnectionPoll {
    None,
    ServerClosed,
    NewConnection(QuicServerConnection, ConnectionId),
}

async fn build_server<C: IntoCertificate, PK: IntoPrivateKey>(
    ip: SocketAddr,
    certificate: C,
    private_key: PK,
) -> Result<Server, Box<dyn Error>> {
    let tls = s2n_quic_tls::Server::builder()
        .with_certificate(certificate, private_key)?
        .build()?;

    let server = Server::builder().with_tls(tls)?.with_io(ip)?.start()?;
    Ok(server)
}

pub enum QuitReason {
    ServerClosed,
    BrokenSender,
}
