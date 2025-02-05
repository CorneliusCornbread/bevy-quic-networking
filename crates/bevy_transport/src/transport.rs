use bevy::prelude::{Component, Query, ResMut, Resource};
use bytes::Bytes;
use std::collections::{vec_deque::Drain, HashMap, VecDeque};

use crate::message::{InboundMessage, OutboundMessage};

/// Based heavily on the transport design from:
/// https://github.com/bacongobbler/bevy_simple_networking/blob/main/src/transport.rs
#[derive(Resource)]
pub struct NetReceiver {
    pub messages: VecDeque<InboundMessage>,
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

#[derive(Component)]
pub struct NetObject {
    pub id: i32,
    pub unprocessed_data: VecDeque<Bytes>,
}

/// Super basic transport, meant more so as a example/base implementation.
/// Not particularly well optimized, least for now.
pub fn rec_messages(mut rec: ResMut<NetReceiver>, mut buffers: Query<&mut NetObject>) {
    // TODO: Here's a better way of doing this, but I'm lazy rn so...
    // Instead of creating a dictionary while we're querying for net objects
    // it may be better to instead turn our dequeue into a dictionary at some
    // point that *isn't* when we have an exclusive hold on our net objects.
    // Whether that's through the receive call when populating NetReceiver or
    // through another system via ECS.

    // TODO: Hashmaps are quite expensive, least the std library one which is
    // cryptographically secure to the best of its abilities.
    // Unsure if this cryptographic security is *entirely* necessary as
    // we may be able to implement cheaper, more informed checks and bounds
    // for this map, such as using rate limiting and input validation.
    let mut message_map: HashMap<i32, VecDeque<Bytes>> = HashMap::new();
    for message in rec.deque_messages() {
        if let Some(pending_messages) = message_map.get_mut(&message.target_id) {
            // TODO: Using the bytes crate may not be necessary.
            // I wonder if we could just passed around an owned buffer of
            // memory instead of a Arc<T> of a buffer by transferring ownership.
            pending_messages.push_back(message.data);
        } else {
            let mut messages = VecDeque::new();
            messages.push_back(message.data);
            message_map.insert(message.target_id, messages);
        }
    }

    for mut buf in buffers.iter_mut() {
        let id = &buf.id;
        if let Some(mut buf_ref) = message_map.remove(id) {
            buf.unprocessed_data.append(&mut buf_ref);
        }
        // Otherwise we have no messages to process for this object
    }
}
