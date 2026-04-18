#[cfg(feature = "serde")]
mod delivery;
#[cfg(feature = "serde")]
mod http_sidecar;
mod mock;
#[cfg(feature = "serde")]
mod void_box;

use crate::contract::{ContractError, RuntimeInspection, StartRequest, StartResult};
use crate::orchestration::{ExecutionRuntime, StructuredOutputResult};

#[cfg(feature = "serde")]
pub use delivery::{DeliveryCapability, MessageDeliveryAdapter, VoidBoxRunRef};
#[cfg(feature = "serde")]
pub use http_sidecar::HttpSidecarAdapter;
pub use mock::MockRuntime;
#[cfg(feature = "serde")]
pub use void_box::VoidBoxRuntimeClient;

#[cfg(feature = "serde")]
use crate::orchestration::{CandidateSpec, InboxSnapshot};
#[cfg(feature = "serde")]
use serde_json::{Map, Value};
#[cfg(feature = "serde")]
use std::collections::BTreeMap;
#[cfg(feature = "serde")]
use std::fs;
#[cfg(feature = "serde")]
use std::path::Path;
#[cfg(feature = "serde")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "serde")]
pub trait ProviderLaunchAdapter {
    fn prepare_launch_request(
        &self,
        request: StartRequest,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> Result<StartRequest, ContractError>;
}

#[cfg(feature = "serde")]
#[derive(Debug, Default, Clone, Copy)]
pub struct LaunchInjectionAdapter;

#[cfg(feature = "serde")]
impl ProviderLaunchAdapter for LaunchInjectionAdapter {
    fn prepare_launch_request(
        &self,
        request: StartRequest,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> Result<StartRequest, ContractError> {
        debug_assert_eq!(candidate.candidate_id, inbox.candidate_id);
        let launch_context = serde_json::to_string(inbox).expect("serialize inbox snapshot");
        let env_overrides = env_provider_overrides();
        let workflow_spec = if candidate.overrides.is_empty() && env_overrides.is_empty() {
            request.workflow_spec.clone()
        } else {
            let mut merged = env_overrides;
            for (key, value) in &candidate.overrides {
                merged.insert(key.clone(), value.clone());
            }
            write_patched_workflow_spec(&request.workflow_spec, &merged)?
        };
        Ok(StartRequest {
            workflow_spec,
            launch_context: Some(launch_context),
            ..request
        })
    }
}

#[cfg(feature = "serde")]
fn env_provider_overrides() -> BTreeMap<String, String> {
    let mut overrides = BTreeMap::new();
    if let Ok(provider) = std::env::var("VOID_CONTROL_LLM_PROVIDER") {
        let trimmed = provider.trim();
        if !trimmed.is_empty() {
            overrides.insert("llm.provider".to_string(), trimmed.to_string());
        }
    }
    overrides
}

#[cfg(feature = "serde")]
fn write_patched_workflow_spec(
    original_path: &str,
    overrides: &BTreeMap<String, String>,
) -> Result<String, ContractError> {
    let original = fs::read_to_string(original_path).map_err(|err| {
        ContractError::new(
            crate::contract::ContractErrorCode::InvalidSpec,
            format!("failed to read workflow template '{original_path}': {err}"),
            false,
        )
    })?;
    let ext = Path::new(original_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("yaml");
    let mut document = if ext.eq_ignore_ascii_case("json") {
        serde_json::from_str::<Value>(&original).map_err(|err| {
            ContractError::new(
                crate::contract::ContractErrorCode::InvalidSpec,
                format!("failed to parse workflow template '{original_path}' as JSON: {err}"),
                false,
            )
        })?
    } else {
        serde_yaml::from_str::<Value>(&original).map_err(|err| {
            ContractError::new(
                crate::contract::ContractErrorCode::InvalidSpec,
                format!("failed to parse workflow template '{original_path}' as YAML: {err}"),
                false,
            )
        })?
    };
    for (path, value) in overrides {
        apply_override_path(&mut document, path, value);
    }
    let rendered = if ext.eq_ignore_ascii_case("json") {
        serde_json::to_string_pretty(&document).map_err(|err| {
            ContractError::new(
                crate::contract::ContractErrorCode::InternalError,
                format!("failed to serialize patched workflow template '{original_path}': {err}"),
                true,
            )
        })?
    } else {
        serde_yaml::to_string(&document).map_err(|err| {
            ContractError::new(
                crate::contract::ContractErrorCode::InternalError,
                format!("failed to serialize patched workflow template '{original_path}': {err}"),
                true,
            )
        })?
    };
    let patched_path = patched_workflow_path(original_path, ext);
    fs::write(&patched_path, rendered).map_err(|err| {
        ContractError::new(
            crate::contract::ContractErrorCode::InternalError,
            format!("failed to write patched workflow template '{patched_path}': {err}"),
            true,
        )
    })?;
    Ok(patched_path)
}

#[cfg(feature = "serde")]
fn patched_workflow_path(original_path: &str, ext: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let candidate = Path::new(original_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("workflow");
    std::env::temp_dir()
        .join(format!("void-control-{candidate}-{nanos}.{ext}"))
        .to_string_lossy()
        .into_owned()
}

#[cfg(feature = "serde")]
fn apply_override_path(root: &mut Value, path: &str, value: &str) {
    let mut cursor = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        let is_leaf = parts.peek().is_none();
        if is_leaf {
            ensure_object(cursor).insert(part.to_string(), Value::String(value.to_string()));
            return;
        }
        let entry = ensure_object(cursor)
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cursor = entry;
    }
}

#[cfg(feature = "serde")]
fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("object value")
}

impl ExecutionRuntime for MockRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        self.start(request)
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        self.inspect(handle)
    }

    fn take_structured_output(&mut self, run_id: &str) -> StructuredOutputResult {
        self.take_structured_output(run_id)
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        if persisted_run_id.starts_with("run-handle:") {
            persisted_run_id.to_string()
        } else {
            format!("run-handle:{persisted_run_id}")
        }
    }
}

#[cfg(feature = "serde")]
impl ExecutionRuntime for VoidBoxRuntimeClient {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        self.start(request)
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        self.inspect(handle)
    }

    fn take_structured_output(&mut self, run_id: &str) -> StructuredOutputResult {
        match self.fetch_structured_output(run_id) {
            Ok(Some(output)) => StructuredOutputResult::Found(output),
            Ok(None) => StructuredOutputResult::Missing,
            Err(err) => StructuredOutputResult::Error(err),
        }
    }

    fn inline_poll_budget(&self) -> usize {
        1
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        if persisted_run_id.starts_with("vb:") {
            persisted_run_id.to_string()
        } else {
            format!("vb:{persisted_run_id}")
        }
    }

    fn delivery_run_ref(&self, handle: &str) -> Option<crate::runtime::VoidBoxRunRef> {
        self.delivery_run_ref(handle).ok()
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::{write_patched_workflow_spec, LaunchInjectionAdapter, ProviderLaunchAdapter};
    use crate::contract::{ExecutionPolicy, StartRequest};
    use crate::orchestration::{CandidateSpec, InboxSnapshot};
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn launch_adapter_patches_yaml_workflow_spec_with_candidate_overrides() {
        let path = temp_workflow_path("yaml");
        fs::write(
            &path,
            r#"api_version: v1
kind: agent
name: example

sandbox:
  mode: auto
  env:
    STRATEGY: baseline

llm:
  provider: claude

agent:
  prompt: baseline prompt
  timeout_secs: 300
"#,
        )
        .expect("write workflow");

        let candidate = CandidateSpec {
            candidate_id: "cand-1".to_string(),
            overrides: BTreeMap::from([
                ("agent.prompt".to_string(), "mutated prompt".to_string()),
                (
                    "sandbox.env.STRATEGY".to_string(),
                    "adaptive-window".to_string(),
                ),
            ]),
        };
        let inbox = InboxSnapshot {
            execution_id: "exec-1".to_string(),
            candidate_id: "cand-1".to_string(),
            iteration: 1,
            entries: Vec::new(),
        };

        let request = LaunchInjectionAdapter
            .prepare_launch_request(
                StartRequest {
                    run_id: "run-1".to_string(),
                    workflow_spec: path.clone(),
                    launch_context: None,
                    policy: sample_policy(),
                },
                &candidate,
                &inbox,
            )
            .expect("prepared request");

        let patched = fs::read_to_string(&request.workflow_spec).expect("read patched workflow");
        assert!(patched.contains("prompt: mutated prompt"));
        assert!(patched.contains("STRATEGY: adaptive-window"));
        assert!(request.launch_context.is_some());
        assert_ne!(request.workflow_spec, path);
    }

    #[test]
    fn patched_workflow_spec_preserves_json_format() {
        let path = temp_workflow_path("json");
        fs::write(
            &path,
            r#"{"api_version":"v1","kind":"agent","name":"example","sandbox":{"mode":"auto","env":{"PROFILE":"baseline"}},"llm":{"provider":"claude"},"agent":{"prompt":"baseline","timeout_secs":300}}"#,
        )
        .expect("write workflow");

        let patched = write_patched_workflow_spec(
            &path,
            &BTreeMap::from([("sandbox.env.PROFILE".to_string(), "wide".to_string())]),
        )
        .expect("patched path");

        let rendered = fs::read_to_string(patched).expect("read patched");
        assert!(rendered.contains("\"PROFILE\": \"wide\""));
    }

    fn temp_workflow_path(ext: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("void-control-launch-adapter-{nanos}.{ext}"))
            .to_string_lossy()
            .into_owned()
    }

    fn sample_policy() -> ExecutionPolicy {
        ExecutionPolicy {
            max_parallel_microvms_per_run: 1,
            max_stage_retries: 0,
            stage_timeout_secs: 600,
            cancel_grace_period_secs: 10,
        }
    }
}
