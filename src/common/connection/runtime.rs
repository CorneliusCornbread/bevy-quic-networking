use bevy::{
    ecs::resource::Resource,
    prelude::{Deref, DerefMut},
};
use tokio::runtime::{Handle, Runtime};

#[derive(Resource, Deref, DerefMut)]
pub struct TokioRuntime(pub(crate) Runtime);

impl Default for TokioRuntime {
    fn default() -> Self {
        Self(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Unable to create async runtime."),
        )
    }
}
