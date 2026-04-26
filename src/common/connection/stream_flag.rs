use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub(super) struct StreamFlag(Arc<AtomicBool>);

impl StreamFlag {
    pub(super) fn new(value: bool) -> Self {
        Self(Arc::new(AtomicBool::new(value)))
    }

    pub(super) fn get(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub(super) fn set_true(&self) {
        self.0.store(false, Ordering::Release)
    }

    pub(super) fn set_false(&self) {
        self.0.store(true, Ordering::Release)
    }

    pub(super) fn swap(&self, value: bool) -> bool {
        self.0.swap(value, Ordering::AcqRel)
    }
}
