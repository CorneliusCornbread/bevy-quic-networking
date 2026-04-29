use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub(crate) struct OpenFlag(Arc<AtomicBool>);

impl OpenFlag {
    pub(crate) fn new(value: bool) -> Self {
        Self(Arc::new(AtomicBool::new(value)))
    }

    pub(crate) fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn set_closed(&self) {
        self.0.store(false, Ordering::Relaxed)
    }

    pub(crate) fn set_open(&self) {
        self.0.store(true, Ordering::Relaxed)
    }
}
