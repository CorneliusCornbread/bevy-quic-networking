use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use aeronet::io::bytes::Bytes;
use ahash::AHasher;
use bevy::{
    ecs::component::Component,
    prelude::{Resource, World},
};
use indexmap::IndexMap;
use s2n_quic::{
    connection::Handle,
    provider::event::events::ConnectionClosed,
    stream::{BidirectionalStream, SendStream},
    Connection, Server,
};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::{
    common::{IntoStreamId, StreamId, TransportData},
    TokioRuntime,
};

const SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

#[derive(Component)]
pub struct QuicServer {
    runtime: tokio::runtime::Handle,
    socket_rec_channel: Receiver<TransportData>,
    socket_send_channel: Sender<Bytes>,
    connection_rec_channel: Receiver<StreamId>,
    stream: Option<Arc<BidirectionalStream>>,
    send_task: JoinHandle<()>,
    rec_task: JoinHandle<()>,
    max_connections: usize,
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

        let (outbount_sender, outbound_receiver) = mpsc::channel(32);
        let (inbound_sender, inbound_receiver) = mpsc::channel(32);
        let (new_conn_sender, new_conn_receiver) = mpsc::channel(32);
        let (bevy_new_conn_sender, bevy_new_conn_receiver) = mpsc::channel(32);

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
            stream: None,
            max_connections,
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
                    let transport = TransportData::ReceivedData(data);
                    sender
                        .send(transport)
                        .await
                        .expect("Error sending received data");
                }
            }
        }
    }

    async fn outbound_send_task(
        mut receiver: Receiver<Bytes>,
        mut new_conns: Receiver<SendStream>,
    ) {
        let mut connections: IndexMap<u64, SendStream> = IndexMap::new();

        'running: loop {
            while let Some(conn) = new_conns.recv().await {
                connections.insert(*conn.stream_id(), conn);
            }

            while let Some(message) = receiver.recv().await {
                let mut i = 0;

                while i < connections.len() {
                    // TODO: error handling/logging
                    let (stream_id, stream) = connections
                        .get_index_mut(i)
                        .expect("Index overflowed connections buffer for send task. Has the connections buffer been modified on another thread?");
                    // TODO: change expect to be more error friendly breaking of while loop instead of a panic

                    let send_res = stream.send(message.clone()).await;

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
                    i += 1;
                }
            }
        }
    }
}

fn server_addr(port: u16) -> SocketAddr {
    SocketAddr::new(SERVER_IP, port)
}
