/// Based heavily on the transport design from:
/// https://github.com/bacongobbler/bevy_simple_networking/blob/main/src/transport.rs
use std::{
    collections::{vec_deque::Drain, VecDeque},
    net::SocketAddr,
};

use bevy::prelude::Resource;
use bytes::Bytes;

pub struct OutboundMessage {
    target: SocketAddr,
    data: Bytes,
}

pub struct InboundMessage {
    data: Bytes,
}

#[derive(Resource)]
pub struct NetReceiver {
    messages: VecDeque<InboundMessage>,
}

#[derive(Resource)]
pub struct NetSender {
    messages: VecDeque<OutboundMessage>,
}

impl NetReceiver {
    pub fn deque_messages(&mut self) -> Vec<InboundMessage> {
        let output = self.messages.drain(..).collect();
        return output;
    }

    pub fn deque_iter(&mut self) -> Drain<'_, InboundMessage> {
        return self.messages.drain(..);
    }

    pub fn push_messages(&mut self, message: InboundMessage) {
        self.messages.push_back(message);
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

impl NetSender {
    pub fn deque_messages(&mut self) -> Vec<OutboundMessage> {
        let output = self.messages.drain(..).collect();
        return output;
    }

    pub fn deque_iter(&mut self) -> Drain<'_, OutboundMessage> {
        return self.messages.drain(..);
    }

    pub fn push_messages(&mut self, message: OutboundMessage) {
        self.messages.push_back(message);
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
