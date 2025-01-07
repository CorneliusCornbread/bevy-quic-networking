use s2n_quic::Connection;
use std::error::Error;

#[derive(Debug)]
pub enum ConnectionState {
    Connected(Connection),
    Failed(Box<dyn Error + Send>),
    InProgress,
}

impl From<Result<Connection, s2n_quic::connection::Error>> for ConnectionState {
    fn from(value: Result<Connection, s2n_quic::connection::Error>) -> Self {
        if let Ok(conn) = value {
            return Self::Connected(conn);
        }

        Self::Failed(Box::new(value.unwrap_err()))
    }
}
