use std::sync::atomic::AtomicU64;

#[derive(Debug, Default)]
pub(crate) struct IdGenerator {
    current: AtomicU64,
}

impl IdGenerator {
    pub fn generate_unique(&mut self) -> u64 {
        self.current
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
