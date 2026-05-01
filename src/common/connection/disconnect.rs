use aeronet_io::{anyhow::anyhow, connection::DisconnectReason};
use s2n_quic::connection::Error as ConnectionError;
use std::{error::Error, sync::Arc};

#[derive(Clone, Debug)]
pub enum ConnectionDisconnectReason {
    /// Connection was closed by the local user explicitly
    UserClosed,
    /// Connection was closed or errored elsewhere
    ConnectionError(s2n_quic::connection::Error),
    MspcChannelClosed {
        channel_name: String,
    },
    InternalError(Arc<dyn Error + Send + Sync>),
}

impl From<Arc<dyn Error + Send + Sync>> for ConnectionDisconnectReason {
    fn from(error: Arc<dyn Error + Send + Sync>) -> Self {
        ConnectionDisconnectReason::InternalError(error)
    }
}

impl From<ConnectionDisconnectReason> for DisconnectReason {
    fn from(val: ConnectionDisconnectReason) -> Self {
        match val {
            ConnectionDisconnectReason::UserClosed => {
                DisconnectReason::ByUser("Connection closed by self".to_string())
            }
            ConnectionDisconnectReason::ConnectionError(error) => {
                DisconnectReason::ByError(anyhow!(
                    "Connection has been closed due to a connection error: {error}"
                ))
            }
            ConnectionDisconnectReason::MspcChannelClosed { channel_name } => {
                DisconnectReason::ByError(anyhow!(
                    "Connection was closed due to an IPC channel \"{channel_name}\" being closed"
                ))
            }
            ConnectionDisconnectReason::InternalError(error) => {
                DisconnectReason::ByError(anyhow!(
                    "Connection was closed due to an internal error: {error}"
                ))
            }
        }
    }
}

pub trait ConnectionErrorDisconnected {
    /// Returns `true` if this error represents a closed or unrecoverable connection.
    fn is_closed(&self) -> bool;
}

impl ConnectionErrorDisconnected for ConnectionError {
    fn is_closed(&self) -> bool {
        matches!(
            self,
            ConnectionError::Closed { .. }
                | ConnectionError::Transport { .. }
                | ConnectionError::Application { .. }
                | ConnectionError::EndpointClosing { .. }
                | ConnectionError::IdleTimerExpired { .. }
                | ConnectionError::NoValidPath { .. }
        )
    }
}
