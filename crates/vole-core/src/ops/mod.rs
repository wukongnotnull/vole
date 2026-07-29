//! 编排骨架：进度事件经 channel 发出，供 CLI/TUI/sidecar 消费。

use crate::vole_proto::StreamEvent;
use crossbeam_channel::Sender;
use thiserror::Error;

use crate::cancel::{CancelToken, Cancelled};

#[derive(Debug, Error)]
pub enum OpsError {
    #[error("操作已取消")]
    Cancelled,
}

impl From<Cancelled> for OpsError {
    fn from(_: Cancelled) -> Self {
        OpsError::Cancelled
    }
}

pub struct Orchestrator {
    cancel: CancelToken,
    events: Option<Sender<StreamEvent>>,
}

impl Orchestrator {
    pub fn new(cancel: CancelToken, events: Option<Sender<StreamEvent>>) -> Self {
        Self { cancel, events }
    }

    pub fn check_cancel(&self) -> Result<(), OpsError> {
        self.cancel.check()?;
        Ok(())
    }

    pub fn emit(&self, event: StreamEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vole_proto::StreamEvent;
    use crossbeam_channel::unbounded;

    #[test]
    fn channel_receives_progress() {
        let (tx, rx) = unbounded();
        let orch = Orchestrator::new(CancelToken::new(), Some(tx));
        orch.emit(StreamEvent::Progress {
            scanned: 42,
            current: ".".into(),
        });
        let ev = rx.try_recv().unwrap();
        assert!(matches!(ev, StreamEvent::Progress { scanned: 42, .. }));
    }

    #[test]
    fn cancel_propagates() {
        let token = CancelToken::new();
        token.cancel();
        let orch = Orchestrator::new(token, None);
        assert!(matches!(orch.check_cancel(), Err(OpsError::Cancelled)));
    }
}
