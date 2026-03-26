use std::fmt::Display;

use crate::common::QuicParentId;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct StreamId {
    parent_id: QuicParentId,
    id: u64,
}

impl Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StreamId(Id: {0}, Parent: {1}, Type: {2:?})",
            self.id,
            self.parent_id.parent_id(),
            self.parent_id.connection_type()
        )
    }
}
