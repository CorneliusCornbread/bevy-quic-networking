use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use bevy::{ecs::component::Component, log::info};
use s2n_quic::{Connection, Server};
use s2n_quic_tls::certificate::{self, IntoCertificate, IntoPrivateKey};
use tokio::{
    runtime::Handle,
    sync::{
        Mutex,
        mpsc::{self, Receiver, Sender},
    },
    task::{JoinError, JoinHandle},
};

use crate::{
    common::{
        connection::{
            QuicConnection,
            id::{ConnectionId, ConnectionIdGenerator},
            runtime::TokioRuntime,
        },
        status_code::StatusCode,
    },
    server::flag::{AtomicPollFlag, PollState},
};

pub mod endpoint;
pub mod flag;

const MAX_PENDING_CONNECTIONS: usize = 100;
const POLL_SLEEP_DUR: Duration = Duration::from_millis(50);

#[derive(Component)]
pub struct QuicServer {
    runtime: Handle,
    server: Arc<Mutex<Server>>,
    id_gen: ConnectionIdGenerator,
}

// TODO: make a function to allow the user to provide their own function to build a server,
// for example providing your own TLS certs.
impl QuicServer {
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

    pub fn poll_connection(&mut self) -> Result<ConnectionPoll, JoinError> {
        let waker = Arc::new(futures::task::noop_waker_ref());
        let mut cx = std::task::Context::from_waker(&waker);

        let mut lock = self.server.blocking_lock();
        let poll = lock.poll_accept(&mut cx);
        drop(lock);

        match poll {
            std::task::Poll::Ready(conn_opt) => {
                if let Some(conn) = conn_opt {
                    let ret = ConnectionPoll::NewConnection(
                        QuicConnection::new(self.runtime.clone(), conn),
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
    NewConnection(QuicConnection, ConnectionId),
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
