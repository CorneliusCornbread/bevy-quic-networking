use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Instant,
};

use aeronet::io::{bytes::Bytes, packet::RecvPacket, Session};
use bevy::{
    ecs::system::{Commands, Query, ResMut, Resource},
    log::error,
    prelude::World,
};
use indexmap::IndexMap;
use s2n_quic::{stream::SendStream, Server};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
    time::Instant as TokioInstant,
};

use crate::{
    common::{IntoStreamId, QuicSessionInternal, StreamId, TransportData},
    TokioRuntime,
};

const SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const CHANNEL_BUFF_SIZE: usize = 64;
const MIN_MTU: usize = 1024;

#[derive(Resource)]
pub struct QuicServer {
    runtime: tokio::runtime::Handle,
    socket_rec_channel: Receiver<TransportData>,
    socket_send_channel: Sender<(Bytes, Option<StreamId>)>,
    connection_rec_channel: Receiver<StreamId>,
    send_task: JoinHandle<()>,
    rec_task: JoinHandle<()>,
    max_connections: usize,
    tracked_connections: IndexMap<StreamId, (bool, Instant)>,
}

impl QuicServer {
    pub fn start_server(
        world: &mut World,
        max_connections: usize,
        port: u16,
    ) -> Result<Self, Box<dyn Error>> {
        let server = Server::builder().with_io(server_addr(port))?.start()?;
        let runtime = world
            .get_resource_or_init::<TokioRuntime>()
            .handle()
            .clone();

        let (outbount_sender, outbound_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (inbound_sender, inbound_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (new_conn_sender, new_conn_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);
        let (bevy_new_conn_sender, bevy_new_conn_receiver) = mpsc::channel(CHANNEL_BUFF_SIZE);

        let send_task = runtime.spawn(async move {
            Self::outbound_send_task(outbound_receiver, new_conn_receiver).await
        });

        let rec_task = runtime.spawn(async move {
            Self::inbound_rec_task(
                server,
                inbound_sender,
                new_conn_sender,
                bevy_new_conn_sender,
            )
            .await
        });

        let quic_server = Self {
            runtime,
            socket_rec_channel: inbound_receiver,
            socket_send_channel: outbount_sender,
            connection_rec_channel: bevy_new_conn_receiver,
            send_task,
            rec_task,
            max_connections,
            tracked_connections: IndexMap::new(),
        };

        Ok(quic_server)
    }

    async fn inbound_rec_task(
        mut server: Server,
        sender: Sender<TransportData>,
        new_conns: Sender<SendStream>,
        bevy_new_conns: Sender<StreamId>,
    ) {
        let mut rec_streams = Vec::new();
        let mut connections = Vec::new();

        'running: loop {
            while let Some(mut con) = server.accept().await {
                let _ = con.keep_alive(true);

                if let Ok(stream) = con.open_bidirectional_stream().await {
                    let (rec, send) = stream.split();
                    rec_streams.push(rec);

                    let new_stream_id = send.stream_id();

                    bevy_new_conns
                        .send(new_stream_id)
                        .await
                        .expect("Error sending new stream to Bevy thread");

                    new_conns
                        .send(send)
                        .await
                        .expect("Error handling new stream");

                    let data = TransportData::Connected(new_stream_id);
                    sender
                        .send(data)
                        .await
                        .expect("Error sending transport stream id");
                } else {
                    // TODO: error codes
                    con.close(99_u32.into());
                }

                connections.push(con);
            }

            for con in rec_streams.iter_mut() {
                if let Ok(Some(data)) = con.receive().await {
                    let packet = RecvPacket {
                        recv_at: TokioInstant::now().into_std(),
                        payload: data,
                    };

                    let transport = TransportData::ReceivedData(packet);
                    sender
                        .send(transport)
                        .await
                        .expect("Error sending received data");
                }
            }
        }
    }

    async fn outbound_send_task(
        mut receiver: Receiver<(Bytes, Option<StreamId>)>,
        mut new_conns: Receiver<SendStream>,
    ) {
        let mut connections: IndexMap<StreamId, SendStream> = IndexMap::new();

        'running: loop {
            while let Some(conn) = new_conns.recv().await {
                connections.insert(conn.stream_id(), conn);
            }

            'message_loop: while let Some(message) = receiver.recv().await {
                // Target message
                if let Some(target_stream_id) = message.1 {
                    if let Some(stream) = connections.get_mut(&target_stream_id) {
                        // TODO: see error handling below, this will need to be handled in the same way
                        let _send_res = stream.send(message.0.clone()).await;
                    }

                    // TODO: log error if we can't find the relevant stream

                    continue 'message_loop;
                }

                // Broadcast message
                let mut i = connections.len();

                while i > 0 {
                    i -= 1;
                    // TODO: error handling/logging
                    let (_stream_id, stream) = connections
                        .get_index_mut(i)
                        .expect("Index overflowed connections buffer for send task. Has the connections buffer been modified on another thread?");
                    // TODO: change expect to be more error friendly breaking of while loop instead of a panic

                    let send_res = stream.send(message.0.clone()).await;

                    // TODO: move this handling to function call
                    if let Err(err) = send_res {
                        match err {
                            s2n_quic::stream::Error::StreamReset { error, source, .. } => {
                                // TODO: retry send later
                            }
                            s2n_quic::stream::Error::InvalidStream { source, .. }
                            | s2n_quic::stream::Error::SendAfterFinish { source, .. } => {
                                // Stream is dead, drop all data for it and remove it
                                connections.swap_remove_index(i);
                            }
                            s2n_quic::stream::Error::MaxStreamDataSizeExceeded {
                                source, ..
                            } => todo!(),
                            s2n_quic::stream::Error::ConnectionError { error, .. } => todo!(),
                            s2n_quic::stream::Error::SendingBlocked { source, .. } => {
                                // TODO: create some smaller message buffer system to handle this case
                            }
                            s2n_quic::stream::Error::NonReadable { source, .. }
                            | s2n_quic::stream::Error::NonWritable { source, .. }
                            | s2n_quic::stream::Error::NonEmptyOutput { source, .. } => {
                                // TODO: unexpected error logging
                            }
                            _ => {
                                // TODO: unexpected error logging
                            }
                        }
                    }
                }
            }
        }
    }
}

fn server_addr(port: u16) -> SocketAddr {
    SocketAddr::new(SERVER_IP, port)
}

/// Drains all the messages between our IO and our Session layer, sending queued data and receiving queued data.
pub(crate) fn drain_messages(
    mut commands: Commands,
    mut sessions: Query<(&mut Session, &QuicSessionInternal)>,
    mut server: ResMut<QuicServer>,
) {
    for entity in sessions.iter_mut() {
        let mut session = entity.0;
        let stream_id = entity.1 .0;

        for data in session.send.drain(..) {
            let send_res = server
                .socket_send_channel
                .blocking_send((data, Some(stream_id)));

            if let Err(e) = send_res {
                error!(
                    "Internal error sending data via socket channel, this data will be not be sent: {}",
                    e
                )
            }
        }

        // blocking_recv will block if there are no messages to be processed until a message is sent
        if server.socket_rec_channel.is_empty() {
            continue;
        }

        while let Some(TransportData::ReceivedData(data)) =
            server.socket_rec_channel.blocking_recv()
        {
            session.recv.push(data);
        }

        while let Some(new_id) = server.connection_rec_channel.blocking_recv() {
            let conn_instant = Instant::now();

            // TODO: add more information about connections
            server
                .tracked_connections
                .insert(new_id, (true, conn_instant));

            commands.spawn((
                Session::new(conn_instant, MIN_MTU),
                QuicSessionInternal(new_id),
            ));
        }
    }
}

pub(crate) fn open_broadcast(mut commands: Commands) {
    use aeronet::io::server::Server as AeronetServer;
    commands.spawn(AeronetServer::new(Instant::now()));
}
