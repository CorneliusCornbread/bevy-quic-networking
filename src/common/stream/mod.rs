use crate::common::{
    attempt::QuicActionAttempt,
    stream::{receive::QuicReceiveStream, send::QuicSendStream},
};

pub mod id;
pub mod receive;
pub mod send;
pub mod session;

pub type QuicReceiveStreamAttempt = QuicActionAttempt<QuicReceiveStream>;
pub type QuicSendStreamAttempt = QuicActionAttempt<QuicReceiveStream>;
pub type QuicBidirectionalStreamAttempt = QuicActionAttempt<(QuicReceiveStream, QuicSendStream)>;
