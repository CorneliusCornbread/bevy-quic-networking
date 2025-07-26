use s2n_quic::Server;
use std::{error::Error, net::SocketAddr};
use tokio::{runtime::Handle, task::JoinHandle};

pub struct QuicServerEndpoint {
    runtime: Handle,
    addr: SocketAddr,
    conn_task: JoinHandle<()>,
}

impl QuicServerEndpoint {
    pub fn new(runtime: Handle, addr: SocketAddr) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder().with_io(addr)?.start()?;

        let conn_task = runtime.spawn(connections_task(server));

        Ok(Self {
            runtime: runtime.clone(),
            addr,
            conn_task,
        })
    }
}

async fn connections_task(server: Server) {
    'running: loop {}
}
