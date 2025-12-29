use std::{error::Error, sync::Arc};

#[derive(Clone)]
pub enum StreamDisconnectReason {
    Closed,
    Reset(s2n_quic::application::Error),
    InvalidStream,
    ConnectionError(s2n_quic::connection::Error),
    ResourceError,
    MspcChannelClosed {
        channel_name: String,
    },
    InternalError(Arc<dyn Error + Send + Sync>),
    /// This should in theory never happen
    NoReason,
}
