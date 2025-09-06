use std::sync::atomic::{AtomicUsize, Ordering};

const STOPPED: i8 = 0;
const POLLING: i8 = 1;

const STOPPED_USIZE: usize = STOPPED as usize;
const POLLING_USIZE: usize = POLLING as usize;

#[derive(Debug)]
pub(crate) struct AtomicPollFlag {
    state: AtomicUsize,
}

impl AtomicPollFlag {
    pub fn new(state: PollState) -> Self {
        Self {
            state: (state as usize).into(),
        }
    }

    /// Loads a value from the atomic flag.
    ///
    /// `load` takes an [`Ordering`] argument which describes the memory ordering of this operation.
    /// Possible values are [`SeqCst`], [`Acquire`] and [`Relaxed`].
    ///
    /// # Panics
    ///
    /// Panics if `order` is [`Release`] or [`AcqRel`].
    pub fn load(&self, order: Ordering) -> PollState {
        self.state
            .load(order)
            .try_into()
            .expect("Poll flag is in invalid state. Has memory corruption occured?")
    }

    /// Stores a value into the atomic flag.
    ///
    /// `store` takes an [`Ordering`] argument which describes the memory ordering of this operation.
    ///  Possible values are [`SeqCst`], [`Release`] and [`Relaxed`].
    ///
    /// # Panics
    ///
    /// Panics if `order` is [`Acquire`] or [`AcqRel`].
    pub fn store(&self, val: PollState, order: Ordering) {
        self.state.store(val as usize, order);
    }

    /// Fetches the value, and applies a function to it that returns an optional
    /// new value. Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`, else
    /// `Err(previous_value)`.
    ///
    /// Note: This may call the function multiple times if the value has been changed from other threads in
    /// the meantime, as long as the function returns `Some(_)`, but the function will have been applied
    /// only once to the stored value.
    ///
    /// `fetch_update` takes two [`Ordering`] arguments to describe the memory ordering of this operation.
    /// The first describes the required ordering for when the operation finally succeeds while the second
    /// describes the required ordering for loads. These correspond to the success and failure orderings of
    pub fn fetch_update<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        f: F,
    ) -> Result<usize, usize>
    where
        F: FnMut(usize) -> Option<usize>,
    {
        self.state.fetch_update(set_order, fetch_order, f)
    }
}

impl Default for AtomicPollFlag {
    fn default() -> Self {
        Self {
            state: STOPPED_USIZE.into(),
        }
    }
}

impl TryFrom<usize> for PollState {
    type Error = RangeError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            STOPPED_USIZE => Ok(PollState::Stopped),
            POLLING_USIZE => Ok(PollState::Polling),
            _ => Err(RangeError(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RangeError(usize);

impl RangeError {
    pub fn get_incorrect_value(&self) -> usize {
        self.0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollState {
    Stopped = STOPPED as isize,
    Polling = POLLING as isize,
}

impl From<PollState> for AtomicPollFlag {
    fn from(value: PollState) -> Self {
        match value {
            PollState::Stopped => Self {
                state: (PollState::Stopped as usize).into(),
            },
            PollState::Polling => Self {
                state: (PollState::Polling as usize).into(),
            },
        }
    }
}
