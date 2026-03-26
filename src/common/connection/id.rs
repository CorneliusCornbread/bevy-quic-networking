use std::fmt;

use crate::common::{QuicParentId, QuicParentType};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct ConnectionId {
    parent_id: QuicParentId,
    id: u64,
}

impl ConnectionId {
    pub fn new(id: u64, parent_id: u64, parent_type: QuicParentType) -> Self {
        Self {
            parent_id: QuicParentId::new(parent_id, parent_type),
            id,
        }
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamId(Id: {0}, Parent: {1}, Type: {2:?})",
            self.id,
            self.parent_id.parent_id(),
            self.parent_id.connection_type()
        )
    }
}
