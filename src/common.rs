use aeronet::io::bytes::Bytes;
use ahash::AHasher;
use bevy::prelude::Deref;
use std::{
    error::Error,
    hash::Hasher,
    net::{IpAddr, SocketAddr},
};

// TODO: Move connect, stream information, and data information into their own enums
#[derive(Debug)]
pub enum TransportData {
    Connected(ConnectionId),
    ConnectFailed(Box<dyn Error + Send>),
    ConnectInProgress,
    FailedStream(Box<dyn Error + Send>),
    ReceivedData(Bytes),
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

#[derive(Deref, Debug)]
pub struct ConnectionId(u64);

impl From<u64> for ConnectionId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Into<u64> for ConnectionId {
    fn into(self) -> u64 {
        self.0
    }
}

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

pub trait AddrHash {
    fn addr_hash(&self, hasher: &mut AHasher) -> ConnectionId;
}

impl AddrHash for SocketAddr {
    fn addr_hash(&self, hasher: &mut AHasher) -> ConnectionId {
        let bytes: IpAddrBytes = self.ip().into();
        match bytes {
            IpAddrBytes::V4(v4) => hasher.write(&v4),
            IpAddrBytes::V6(v6) => hasher.write(&v6),
        }

        ConnectionId(hasher.finish())
    }
}

impl AddrHash for IpAddr {
    fn addr_hash(&self, hasher: &mut AHasher) -> ConnectionId {
        let bytes: IpAddrBytes = (*self).into();
        match bytes {
            IpAddrBytes::V4(v4) => hasher.write(&v4),
            IpAddrBytes::V6(v6) => hasher.write(&v6),
        }

        ConnectionId(hasher.finish())
    }
}
