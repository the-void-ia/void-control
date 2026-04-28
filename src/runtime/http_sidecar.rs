#[cfg(feature = "serde")]
use std::io;

#[cfg(feature = "serde")]
use async_trait::async_trait;

#[cfg(feature = "serde")]
use crate::contract::{ContractError, ContractErrorCode};
#[cfg(feature = "serde")]
use crate::orchestration::{CandidateSpec, CommunicationIntent, InboxSnapshot};

#[cfg(feature = "serde")]
use serde_json::Value;

#[cfg(feature = "serde")]
use super::daemon_address::default_unix_url;
#[cfg(feature = "serde")]
use super::delivery::{DeliveryCapability, MessageDeliveryAdapter, VoidBoxRunRef};
#[cfg(feature = "serde")]
use super::void_box::{build_transport, HttpResponse, HttpTransport};

#[cfg(feature = "serde")]
pub struct HttpSidecarAdapter {
    transport: Box<dyn HttpTransport + Send + Sync>,
}

#[cfg(feature = "serde")]
impl HttpSidecarAdapter {
    /// Construct a sidecar adapter that talks to the daemon at the
    /// auto-discovered AF_UNIX socket. Uses the same scheme dispatch and
    /// token resolution as [`crate::runtime::VoidBoxRuntimeClient::new`].
    pub fn new() -> Self {
        Self::with_daemon_url(default_unix_url())
    }

    /// Construct a sidecar adapter pinned to an explicit daemon URL.
    ///
    /// Same dispatch contract as `VoidBoxRuntimeClient::new`:
    /// - `unix:///abs/path` → AF_UNIX, no auth header.
    /// - `http://host:port` (or bare `host:port`) → TCP with bearer token
    ///   resolved from `VOIDBOX_DAEMON_TOKEN_FILE`, `VOIDBOX_DAEMON_TOKEN`,
    ///   or `$XDG_CONFIG_HOME/voidbox/daemon-token`. Construction panics if
    ///   TCP is configured and no token resolves.
    pub fn with_daemon_url(daemon_url: String) -> Self {
        let url = if daemon_url.trim().is_empty() {
            default_unix_url()
        } else {
            daemon_url
        };
        let transport = build_transport(&url).unwrap_or_else(|err| {
            panic!("HttpSidecarAdapter construction failed: {err}");
        });
        Self { transport }
    }

    async fn request(
        &self,
        _run: &VoidBoxRunRef,
        method: &str,
        path: &str,
        body: &str,
    ) -> io::Result<HttpResponse> {
        // The transport is bound at construction; `run.daemon_base_url` is
        // retained on the run-ref for forward-compat with multi-daemon
        // deployments but not consulted here. Same-process daemon URL is
        // assumed stable for the lifetime of the adapter.
        self.transport
            .request(method, path, body)
            .await
            .map_err(contract_error_to_io)
    }
}

#[cfg(feature = "serde")]
impl Default for HttpSidecarAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "serde")]
#[async_trait(?Send)]
impl MessageDeliveryAdapter for HttpSidecarAdapter {
    fn capabilities(&self) -> Vec<DeliveryCapability> {
        vec![
            DeliveryCapability::LaunchInjection,
            DeliveryCapability::LivePoll,
        ]
    }

    async fn inject_at_launch(
        &self,
        run: &VoidBoxRunRef,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> io::Result<()> {
        debug_assert_eq!(candidate.candidate_id, inbox.candidate_id);
        let body = serde_json::to_string(inbox).map_err(io::Error::other)?;
        let path = format!("/v1/runs/{}/inbox", run.run_id);
        let response = self.request(run, "PUT", &path, &body).await?;
        if response.status >= 400 {
            return Err(io::Error::other(format!(
                "void-box inbox injection failed: HTTP {}",
                response.status
            )));
        }
        Ok(())
    }

    async fn drain_intents(&self, run: &VoidBoxRunRef) -> io::Result<Vec<CommunicationIntent>> {
        let path = format!("/v1/runs/{}/intents", run.run_id);
        let response = self.request(run, "GET", &path, "").await?;
        if response.status == 404 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("run '{}' not found", run.run_id),
            ));
        }
        if response.status >= 400 {
            return Err(io::Error::other(format!(
                "void-box intent drain failed: HTTP {}",
                response.status
            )));
        }
        if response.body.trim().is_empty() {
            return Ok(Vec::new());
        }

        let value: Value = serde_json::from_str(&response.body).map_err(io::Error::other)?;
        let intents_value = match value {
            Value::Array(_) => value,
            Value::Object(ref object) => object.get("intents").cloned().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "intents response missing array")
            })?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "intents response must be an array",
                ))
            }
        };

        serde_json::from_value(intents_value).map_err(io::Error::other)
    }

    fn messaging_skill(&self, _run: &VoidBoxRunRef) -> String {
        [
            "# Collaboration Protocol",
            "",
            "You are part of a multi-agent execution.",
            "",
            "## Reading messages",
            "GET http://10.0.2.2:8090/v1/inbox",
            "",
            "## Sending messages",
            "POST http://10.0.2.2:8090/v1/intents",
            "Content-Type: application/json",
            "",
            "{\"kind\": \"proposal\", \"audience\": \"broadcast\",",
            " \"payload\": {\"summary_text\": \"...\"}, \"priority\": \"normal\"}",
            "",
            "## Message kinds",
            "- proposal: concrete solution or approach",
            "- signal: observation other agents should know",
            "- evaluation: assessment of another agent's proposal",
            "",
            "## Audience",
            "- broadcast: all agents",
            "- leader: coordinator only",
        ]
        .join("\n")
    }
}

#[cfg(feature = "serde")]
fn contract_error_to_io(err: ContractError) -> io::Error {
    let kind = match err.code {
        ContractErrorCode::NotFound => io::ErrorKind::NotFound,
        ContractErrorCode::InvalidSpec => io::ErrorKind::InvalidData,
        ContractErrorCode::InvalidPolicy => io::ErrorKind::InvalidInput,
        ContractErrorCode::AlreadyTerminal => io::ErrorKind::Other,
        ContractErrorCode::ResourceLimitExceeded => io::ErrorKind::Other,
        ContractErrorCode::StructuredOutputMissing => io::ErrorKind::NotFound,
        ContractErrorCode::StructuredOutputMalformed => io::ErrorKind::InvalidData,
        ContractErrorCode::ArtifactNotFound => io::ErrorKind::NotFound,
        ContractErrorCode::ArtifactPublicationIncomplete => io::ErrorKind::Other,
        ContractErrorCode::ArtifactStoreUnavailable => io::ErrorKind::Other,
        ContractErrorCode::RetrievalTimeout => io::ErrorKind::TimedOut,
        ContractErrorCode::InternalError => io::ErrorKind::Other,
    };
    io::Error::new(kind, err.message)
}
