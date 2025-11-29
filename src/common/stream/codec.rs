use std::sync::mpsc::Sender;

use aeronet::io::packet::RecvPacket;
use bevy::log::info;
use bytes::Buf;
use bytes::BufMut;
use bytes::Bytes;

use bytes::BytesMut;
use s2n_quic::stream::ReceiveStream;

const HEADER_LEN: usize = size_of::<u32>();
const BUFF_SIZE: usize = 100;

pub(crate) struct DecodedReceiveStream {
    pub stream: ReceiveStream,
    pub sender: Sender<RecvPacket>,
    decode_state: DecoderState,
    read_buf: [Bytes; BUFF_SIZE],
}

pub(crate) enum DecoderState {
    ReadLen {
        buf: [u8; HEADER_LEN],
        filled: usize,
    },
    ReadBody(BodyData),
}

struct BodyData {
    len: usize,
    filled: usize,
    parts: Vec<Bytes>,
}

impl BodyData {
    pub(crate) fn take_bytes(&mut self, bytes_buf: &mut [Bytes], count: usize) {
        for data in &mut bytes_buf[0..count] {
            let payload = std::mem::take(data);
            if payload.is_empty() {
                info!("Body data received zero length byte buffer");
                continue;
            }

            self.parts.push(payload);
        }
    }

    // TODO: Implement configurable limits to how large a single frame can be
    pub(crate) fn build_packet_bytes(&mut self) -> Option<(Bytes, DecoderState)> {
        if self.len > self.filled {
            return None;
        }

        let mut combined = BytesMut::with_capacity(self.filled);

        for part in &self.parts {
            combined.extend_from_slice(part);
        }

        let message_body = combined.split_to(self.len).freeze();

        let next_state = if combined.len() >= HEADER_LEN {
            let slice = &combined[..HEADER_LEN];

            // We have enough bytes to read the next message length
            let len = read_length(slice);
            combined.advance(HEADER_LEN);

            DecoderState::ReadBody(BodyData {
                len,
                filled: combined.len(),
                parts: vec![combined.freeze()],
            })
        } else if combined.is_empty() {
            // No extra bytes, start fresh
            DecoderState::ReadLen {
                buf: [0u8; HEADER_LEN],
                filled: 0,
            }
        } else {
            // Partial header bytes
            let mut buf = [0u8; HEADER_LEN];
            buf[..combined.len()].copy_from_slice(&combined);

            DecoderState::ReadLen {
                buf,
                filled: combined.len(),
            }
        };

        Some((message_body, next_state))
    }
}

// Example length parsing function (adjust to your protocol)
fn read_length(bytes: &[u8]) -> usize {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
}
