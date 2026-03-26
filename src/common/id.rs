use std::sync::atomic::{AtomicU64, Ordering};

static GLOBAL_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default)]
pub(crate) struct IdGenerator;

impl IdGenerator {
    pub fn generate_unique() -> u64 {
        GLOBAL_ID.fetch_add(1, Ordering::Relaxed)
    }
}
