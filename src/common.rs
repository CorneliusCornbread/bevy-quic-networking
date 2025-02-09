use ahash::AHasher;
use bevy::prelude::Deref;
use s2n_quic::{stream::BidirectionalStream, Connection};
use std::{
    error::Error,
    hash::Hasher,
    net::{IpAddr, SocketAddr},
};

// TODO: Move connect, stream information, and data information into their own enums
#[derive(Debug)]
pub enum TransportData {
    Connected(Connection),
    ConnectFailed(Box<dyn Error + Send>),
    ConnectInProgress,
    NewStream(BidirectionalStream),
    FailedStream(Box<dyn Error + Send>),
}

impl From<Result<Connection, s2n_quic::connection::Error>> for TransportData {
    fn from(value: Result<Connection, s2n_quic::connection::Error>) -> Self {
        if let Ok(conn) = value {
            return Self::Connected(conn);
        }

        Self::ConnectFailed(Box::new(value.unwrap_err()))
    }
}

impl From<Result<BidirectionalStream, s2n_quic::connection::Error>> for TransportData {
    fn from(value: Result<BidirectionalStream, s2n_quic::connection::Error>) -> Self {
        if let Ok(stream) = value {
            return Self::NewStream(stream);
        }

        Self::FailedStream(Box::new(value.unwrap_err()))
    }
}

impl TransportData {
    pub fn try_keep_alive(&mut self, keep_alive: bool) {
        if let TransportData::Connected(conn) = self {
            conn.keep_alive(keep_alive);
        }
    }
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

#[derive(Deref)]
pub struct ConnectionId(pub u64);

impl From<IpAddr> for ConnectionId {
    fn from(value: IpAddr) -> Self {
        let bytes: IpAddrBytes = value.into();
        let mut hasher = AHasher::default();

        match bytes {
            IpAddrBytes::V4(v4) => {
                hasher.write(&v4);
                hasher.finish().into()
            }
            IpAddrBytes::V6(v6) => {
                hasher.write(&v6);
                hasher.finish().into()
            }
        }
    }
}

impl From<SocketAddr> for ConnectionId {
    fn from(value: SocketAddr) -> Self {
        let mut hasher = AHasher::default();
        let bytes: IpAddrBytes = value.ip().into();
        match bytes {
            IpAddrBytes::V4(v4) => hasher.write(&v4),
            IpAddrBytes::V6(v6) => hasher.write(&v6),
        }
        ConnectionId(hasher.finish())
    }
}

impl From<u64> for ConnectionId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
