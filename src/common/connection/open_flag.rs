use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub(super) struct OpenFlag(Arc<AtomicBool>);

impl OpenFlag {
    pub(super) fn new(value: bool) -> Self {
        Self(Arc::new(AtomicBool::new(value)))
    }

    pub(super) fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub(super) fn set_closed(&self) {
        self.0.store(false, Ordering::Relaxed)
    }

    pub(super) fn set_open(&self) {
        self.0.store(true, Ordering::Relaxed)
    }
}
