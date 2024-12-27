use bytes::Bytes;
use std::net::SocketAddr;

pub enum Reliability {
    Reliable,
    Unreliable,
}

pub enum Ordering {
    Ordered,
    Unordered,
}

pub struct OutboundMessage {
    pub network_target: SocketAddr,
    pub target_id: i32,
    pub data: Bytes,
    pub reliability: Reliability,
    pub ordering: Ordering,
}

pub struct InboundMessage {
    pub network_sender: SocketAddr,
    pub target_id: i32,
    pub data: Bytes,
}
