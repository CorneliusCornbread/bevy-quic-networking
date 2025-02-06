use s2n_quic::{stream::BidirectionalStream, Connection};
use std::error::Error;

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
