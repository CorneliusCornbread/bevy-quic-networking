use aeronet_io::{anyhow::anyhow, connection::DisconnectReason};
use s2n_quic::connection::{Error as ConnectionError, error::Code};
use std::{error::Error, sync::Arc};

const PEER_CLOSED_WITHOUT_CODE: &str =
    "Connection has been closed by user without an error";

#[derive(Clone, Debug)]
pub enum ConnectionDisconnectReason {
    /// Connection was closed by the local user explicitly
    UserClosed(Code),
    /// Connection was closed by the peer without an error
    PeerClosed,
    /// Connection was closed or errored elsewhere
    ConnectionError(ConnectionError),
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
            ConnectionDisconnectReason::UserClosed(code) => DisconnectReason::ByUser(
                format!("Connection closed by user with error code {}", code),
            ),
            ConnectionDisconnectReason::PeerClosed => {
                DisconnectReason::ByPeer(PEER_CLOSED_WITHOUT_CODE.to_owned())
            }
            ConnectionDisconnectReason::ConnectionError(conn_err) => match conn_err {
                s2n_quic::connection::Error::Application {
                    error, initiator, ..
                } => match initiator {
                    s2n_quic::provider::event::Location::Local => {
                        DisconnectReason::ByUser(format!(
                            "Connection has been closed by user with error code: {}",
                            error
                        ))
                    }
                    s2n_quic::provider::event::Location::Remote => {
                        DisconnectReason::ByPeer(format!(
                            "Connection has been closed by peer with error code: {}",
                            error
                        ))
                    }
                },
                s2n_quic::connection::Error::Closed { initiator, .. } => {
                    match initiator {
                        s2n_quic::provider::event::Location::Local => {
                            DisconnectReason::ByUser(PEER_CLOSED_WITHOUT_CODE.to_owned())
                        }
                        s2n_quic::provider::event::Location::Remote => {
                            DisconnectReason::ByPeer(
                                "Connection has been closed by peer without an error"
                                    .to_owned(),
                            )
                        }
                    }
                }
                s2n_quic::connection::Error::Transport {
                    code,
                    reason,
                    initiator,
                    ..
                } => match initiator {
                    s2n_quic::provider::event::Location::Local => {
                        DisconnectReason::ByUser(format!(
                            "Connection has been closed at the transport level by the user with the code: {}, with the reason {}",
                            code, reason
                        ))
                    }
                    s2n_quic::provider::event::Location::Remote => {
                        DisconnectReason::ByPeer(format!(
                            "Connection has been closed at the transport level by the peer with the code: {}, with the reason {}",
                            code, reason
                        ))
                    }
                },
                s2n_quic::connection::Error::EndpointClosing { .. } => {
                    DisconnectReason::ByUser("Local endpoint closing".to_owned())
                }
                _ => DisconnectReason::ByError(anyhow!(
                    "Connection has been closed due to a connection error: {conn_err}"
                )),
            },
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
