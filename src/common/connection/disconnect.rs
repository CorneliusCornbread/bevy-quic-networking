use std::{error::Error, sync::Arc};

use aeronet_io::{anyhow::anyhow, connection::DisconnectReason};

#[derive(Clone, Debug)]
pub enum ConnectionDisconnectReason {
    UserClosed,
    PeerClosed,
    ConnectionError(s2n_quic::connection::Error),
    MspcChannelClosed {
        channel_name: String,
    },
    InternalError(Arc<dyn Error + Send + Sync>),
    /// This should in theory never happen
    NoReason,
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
                DisconnectReason::ByUser("Connection closed by self.".to_owned())
            }
            ConnectionDisconnectReason::PeerClosed => {
                DisconnectReason::ByPeer("Connection closed by peer.".to_owned())
            }
            ConnectionDisconnectReason::ConnectionError(error) => DisconnectReason::ByError(
                anyhow!("Connection has been closed due to a connection error: {error}"),
            ),
            ConnectionDisconnectReason::MspcChannelClosed { channel_name } => {
                DisconnectReason::ByError(anyhow!(
                    "Connection was closed due to an IPC channel \"{channel_name}\" being closed"
                ))
            }
            ConnectionDisconnectReason::InternalError(error) => DisconnectReason::ByError(anyhow!(
                "Connection was closed due to an internal error: {error}"
            )),
            ConnectionDisconnectReason::NoReason => DisconnectReason::ByError(anyhow!(
                "Connection was closed without reason, this is a bug :("
            )),
        }
    }
}
