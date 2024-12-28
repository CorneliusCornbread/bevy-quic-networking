use bevy::prelude::Resource;

pub const DEFAULT_TICK_RATE: u16 = 32;

#[derive(Resource, Clone)]
pub struct NetworkConfig {
    tick_rate: u16,
}

impl NetworkConfig {
    pub fn new(tick_rate: u16) -> Self {
        Self { tick_rate }
    }

    pub fn tick_rate(&self) -> u16 {
        self.tick_rate
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            tick_rate: DEFAULT_TICK_RATE,
        }
    }
}
