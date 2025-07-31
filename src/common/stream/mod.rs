use bevy::ecs::component::Component;
use s2n_quic::stream::Stream;

use crate::common::{
    attempt::QuicActionAttempt,
    stream::{receive::QuicReceiveStream, send::QuicSendStream},
};

pub mod receive;
pub mod send;
pub mod session;

pub type QuicReceiveStreamAttempt = QuicActionAttempt<QuicReceiveStream>;
pub type QuicSendStreamAttempt = QuicActionAttempt<QuicReceiveStream>;
pub type QuicBidirectionalStreamAttempt = QuicActionAttempt<(QuicReceiveStream, QuicSendStream)>;
