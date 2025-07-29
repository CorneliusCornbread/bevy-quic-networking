use s2n_quic::{Connection, Server};
use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, atomic::AtomicBool},
};
use tokio::{
    runtime::Handle,
    sync::mpsc::{Receiver, Sender, channel, error::TrySendError},
    task::JoinHandle,
};

use crate::status_code::StatusCode;

const NEW_CONN_BUFF_SIZE: usize = 32;

pub struct QuicServerEndpoint {
    runtime: Handle,
    conn_task: JoinHandle<()>,
    shutdown_flag: Arc<AtomicBool>,
    connection_rec: Receiver<Connection>,
}

impl QuicServerEndpoint {
    pub fn new(runtime: Handle, server: Server) -> Result<Self, Box<dyn Error>> {
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let (connection_sender, connection_rec) = channel(NEW_CONN_BUFF_SIZE);

        let conn_task = runtime.spawn(connections_task(
            server,
            connection_sender,
            shutdown_flag.clone(),
        ));

        let endpoint = Self {
            runtime: runtime.clone(),
            conn_task,
            shutdown_flag,
            connection_rec,
        };

        Ok(endpoint)
    }
}

async fn connections_task(
    mut server: Server,
    sender: Sender<Connection>,
    shutdown_flag: Arc<AtomicBool>,
) {
    'running: loop {
        if let Some(conn) = server.accept().await {
            let send_res = sender.try_send(conn);

            if let Err(e) = send_res {
                match e {
                    TrySendError::Full(failed_conn) => {
                        // Close the connection with a message indicating we are overloaded
                        failed_conn.close(StatusCode::ServiceUnavailable.into());
                    }
                    TrySendError::Closed(failed_conn) => {
                        // Close the connection with an internal error being the cause
                        failed_conn.close(StatusCode::InternalServerError.into());
                    }
                }
            }
        } else {
            // Only returns None when the server has been 'closed'
            // unsure what the hell that means because there's no close() function of any sort
            break 'running;
        }

        if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break 'running;
        }
    }
}
