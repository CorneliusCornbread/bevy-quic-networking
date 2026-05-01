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

    #[allow(dead_code)] // There's a possibility we will need this in the future, no real reason to get rid of it for now
    pub(crate) fn set_open(&self) {
        self.0.store(true, Ordering::Relaxed)
    }
}
