pub enum ConnectionDisconnectReason {
    ConnectionClosed(s2n_quic::application::Error),
}
