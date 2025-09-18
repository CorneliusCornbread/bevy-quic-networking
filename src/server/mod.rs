use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use bevy::{
    ecs::component::Component,
    log::{info, warn},
};
use s2n_quic::{Connection, Server};
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
    poll_flag: Arc<AtomicPollFlag>,
    poll_job: JoinHandle<()>,
    poll_rec: Receiver<Connection>,
}

impl QuicServer {
    pub fn new(runtime: Handle, server: Server) -> Self {
        let server_mutex = Arc::new(Mutex::new(server));
        let poll_flag: Arc<AtomicPollFlag> = Default::default();
        let (send, rec) = mpsc::channel(MAX_PENDING_CONNECTIONS);
        let job = runtime.spawn(accept_connection(
            server_mutex.clone(),
            send,
            poll_flag.clone(),
        ));

        Self {
            runtime,
            server: server_mutex,
            id_gen: Default::default(),
            poll_flag,
            poll_job: job,
            poll_rec: rec,
        }
    }

    pub fn poll_connection(&mut self) -> Result<ConnectionPoll, JoinError> {
        if !self.poll_job.is_finished() {
            return Ok(ConnectionPoll::ServerClosed);
        }

        if let Some(conn) = self.poll_rec.blocking_recv() {
            return Ok(ConnectionPoll::NewConnection(
                QuicConnection::new(self.runtime.clone(), conn),
                self.id_gen.generate_id(),
            ));
        }

        Ok(ConnectionPoll::ServerClosed)
    }

    pub fn is_polling(&self) -> bool {
        match self.poll_flag.load(Ordering::Acquire) {
            PollState::Stopped => false,
            PollState::Polling => true,
        }
    }

    pub fn set_polling(&mut self, polling: PollState) {
        self.poll_flag.store(polling, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub enum ConnectionPoll {
    None,
    ServerClosed,
    NewConnection(QuicConnection, ConnectionId),
}

async fn accept_connection(
    server: Arc<Mutex<Server>>,
    sender: Sender<Connection>,
    flag: Arc<AtomicPollFlag>,
) {
    loop {
        let poll = flag.load(Ordering::Acquire);

        if poll != PollState::Polling {
            tokio::time::sleep(POLL_SLEEP_DUR).await;
            continue;
        }

        let mut lock = server.lock().await;
        let conn_opt = lock.accept().await;
        drop(lock);

        if let Some(conn) = conn_opt {
            let res = sender.send(conn).await;

            if let Err(e) = res {
                warn!(
                    "Unable to send a received connection. What happened to our receivers? Quitting poll task."
                );

                e.0.close(StatusCode::ServiceUnavailable.into());

                break;
            }
        } else {
            info!("Server connection poll returned none, shutting poll task down");
            break;
        }
    }
}
