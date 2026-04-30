use bevy::app::Plugin;

use crate::common::runtime::TokioRuntime;

pub const DEFAULT_TICK_RATE: u16 = 64;

pub struct QuicAsyncPlugin {
    tick_rate: u16,
}

impl Plugin for QuicAsyncPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<TokioRuntime>();
    }
}

impl Default for QuicAsyncPlugin {
    fn default() -> Self {
        Self {
            tick_rate: DEFAULT_TICK_RATE,
        }
    }
}

impl QuicAsyncPlugin {
    pub fn tick_rate(&self) -> u16 {
        self.tick_rate
    }

    pub fn new(tick_rate: u16) -> Self {
        Self { tick_rate }
    }
}
