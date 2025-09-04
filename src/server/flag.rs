use std::{
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

const STOPPED: i8 = 0;
const STOPPING: i8 = 1;
const POLLING: i8 = 2;

const STOPPED_USIZE: usize = STOPPED as usize;
const STOPPING_USIZE: usize = STOPPING as usize;
const POLLING_USIZE: usize = POLLING as usize;

pub struct AtomicPollFlag {
    state: AtomicUsize,
}

impl AtomicPollFlag {
    pub fn new(state: PollState) -> Self {
        Self {
            state: (state as usize).into(),
        }
    }

    pub fn load(&self, order: Ordering) -> PollState {
        self.state
            .load(order)
            .try_into()
            .expect("Poll flag is in invalid state. Has memory corruption occured?")
    }

    pub fn fetch_update(&mut self, val: PollState, order: Ordering) -> PollState {
        todo!()
    }
}

impl TryFrom<usize> for PollState {
    type Error = RangeError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            STOPPED_USIZE => Ok(PollState::Stopped),
            STOPPING_USIZE => Ok(PollState::Stopping),
            POLLING_USIZE => Ok(PollState::Polling),
            _ => Err(RangeError(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RangeError(usize);

pub enum PollState {
    Stopped = STOPPED as isize,
    Stopping = STOPPING as isize,
    Polling = POLLING as isize,
}

impl From<PollState> for AtomicPollFlag {
    fn from(value: PollState) -> Self {
        match value {
            PollState::Stopped => Self {
                state: (PollState::Stopped as usize).into(),
            },
            PollState::Stopping => Self {
                state: (PollState::Stopping as usize).into(),
            },
            PollState::Polling => Self {
                state: (PollState::Polling as usize).into(),
            },
        }
    }
}
