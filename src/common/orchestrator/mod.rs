use tokio::{runtime::Handle, task::JoinHandle};

pub(crate) struct AsyncOrchestrator {
    runtime: Handle,
    task_join: JoinHandle<()>,
}

impl AsyncOrchestrator {}

struct AsyncOrchestratorTask {}

impl AsyncOrchestratorTask {}
