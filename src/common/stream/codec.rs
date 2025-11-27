use std::sync::mpsc::Sender;

use aeronet::io::packet::RecvPacket;
pub use bytes::Bytes;

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
    fn build_packet_bytes(&mut self) -> Option<Bytes> {
        if self.len > self.filled {
            return None;
        }

        // self.len <= self.filled
        todo!()
    }
}
