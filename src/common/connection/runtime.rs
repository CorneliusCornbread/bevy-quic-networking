use bevy::{
    ecs::resource::Resource,
    prelude::{Deref, DerefMut},
};
use tokio::runtime::Runtime;

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct TokioRuntime(pub(crate) Runtime);

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
