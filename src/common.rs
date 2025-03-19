use aeronet::io::packet::RecvPacket;
use bevy::prelude::Deref;
use s2n_quic::stream::SendStream;
use std::error::Error;
use std::net::IpAddr;

// TODO: Move connect, stream information, and data information into their own enums
#[derive(Debug)]
pub enum TransportData {
    Connected(StreamId),
    ConnectFailed(Box<dyn Error + Send>),
    ConnectInProgress,
    FailedStream(Box<dyn Error + Send>),
    ReceivedData(RecvPacket),
}

pub enum IpAddrBytes {
    V4([u8; 4]),
    V6([u8; 16]),
}

impl IpAddrBytes {
    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            IpAddrBytes::V4(ipv4_addr) => ipv4_addr.to_vec(),
            IpAddrBytes::V6(ipv6_addr) => ipv6_addr.to_vec(),
        }
    }
}

impl From<IpAddr> for IpAddrBytes {
    fn from(value: IpAddr) -> Self {
        match value {
            IpAddr::V4(ipv4_addr) => IpAddrBytes::V4(ipv4_addr.octets()),
            IpAddr::V6(ipv6_addr) => IpAddrBytes::V6(ipv6_addr.octets()),
        }
    }
}

#[derive(Deref, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct StreamId(u64);

impl From<&SendStream> for StreamId {
    fn from(value: &SendStream) -> Self {
        value.stream_id()
    }
}

impl From<u64> for StreamId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

pub trait IntoStreamId {
    fn stream_id(&self) -> StreamId;
}

impl IntoStreamId for SendStream {
    fn stream_id(&self) -> StreamId {
        StreamId(self.id())
    }
}
