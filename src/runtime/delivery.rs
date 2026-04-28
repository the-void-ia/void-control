#[cfg(feature = "serde")]
use std::io;

#[cfg(feature = "serde")]
use async_trait::async_trait;

#[cfg(feature = "serde")]
use crate::orchestration::{CandidateSpec, CommunicationIntent, InboxEntry, InboxSnapshot};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryCapability {
    LaunchInjection,
    RestoreInjection,
    LivePush,
    LivePoll,
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoidBoxRunRef {
    pub daemon_base_url: String,
    pub run_id: String,
}

#[cfg(feature = "serde")]
#[async_trait(?Send)]
pub trait MessageDeliveryAdapter {
    fn capabilities(&self) -> Vec<DeliveryCapability>;

    async fn inject_at_launch(
        &self,
        run: &VoidBoxRunRef,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> io::Result<()>;

    /// Drain intents from the transport buffer.
    /// This is non-idempotent by contract: the second drain observes an empty buffer.
    async fn drain_intents(&self, _run: &VoidBoxRunRef) -> io::Result<Vec<CommunicationIntent>> {
        Ok(Vec::new())
    }

    async fn push_live(&self, _run: &VoidBoxRunRef, _message: &InboxEntry) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "live push is unsupported",
        ))
    }

    fn messaging_skill(&self, _run: &VoidBoxRunRef) -> String;
}
