use std::{error::Error, sync::Arc};
use tokio::{runtime::Handle, task::JoinHandle};

#[derive(Debug)]
pub(crate) struct QuicTaskState<T>
where
    T: Clone + From<Arc<dyn Error + Send + Sync>>,
{
    task: Option<JoinHandle<T>>,
    disconnect_reason: Option<T>,
    runtime: Handle,
}

impl<T> QuicTaskState<T>
where
    T: Clone + From<Arc<dyn Error + Send + Sync>>,
{
    pub fn new(runtime: Handle, task: JoinHandle<T>) -> Self {
        Self {
            task: Some(task),
            disconnect_reason: None,
            runtime,
        }
    }

    pub fn is_finished(&self) -> bool {
        if let Some(join) = &self.task {
            return join.is_finished();
        }
        true
    }
}

impl<T: Clone> QuicTaskState<T>
where
    T: From<Arc<dyn Error + Send + Sync>>,
{
    pub fn get_disconnect_reason(&mut self) -> Option<T> {
        if let Some(join_ref) = self.task.as_ref() {
            if !join_ref.is_finished() {
                return None;
            }
            let join = self.task.take().unwrap();
            let join_res = self.runtime.block_on(join);
            if let Err(reason) = join_res {
                self.disconnect_reason = Some(T::from(Arc::new(reason)));
                return self.disconnect_reason.clone();
            }
            self.disconnect_reason = Some(join_res.unwrap());
            return self.disconnect_reason.clone();
        }
        if self.disconnect_reason.is_none() {
            #[cfg(debug_assertions)]
            panic!(
                "{} is in invalid state, neither a join handle nor a disconnect reason was found.",
                std::any::type_name::<T>()
            );

            #[cfg(not(debug_assertions))]
            bevy::log::error!(
                "{} task is in invalid state, neither a join handle nor a disconnect reason was found. Returning none, this may result in weird behaviour.",
                std::any::type_name::<T>()
            );
        }
        self.disconnect_reason.clone()
    }
}
