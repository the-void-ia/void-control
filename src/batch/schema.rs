use std::error::Error;
use std::fmt::{Display, Formatter};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes optional batch metadata.
pub struct BatchMetadata {
    #[cfg_attr(feature = "serde", serde(default))]
    pub name: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes the worker defaults used for every batch job.
pub struct BatchWorker {
    pub template: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub provider: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes optional batch execution preferences.
pub struct BatchMode {
    #[cfg_attr(feature = "serde", serde(default))]
    pub parallelism: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub background: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub interaction: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one background job in a batch.
pub struct BatchJob {
    #[cfg_attr(feature = "serde", serde(default))]
    pub name: Option<String>,
    pub prompt: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes a high-level batch or yolo submission.
pub struct BatchSpec {
    pub api_version: String,
    pub kind: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub metadata: Option<BatchMetadata>,
    pub worker: BatchWorker,
    #[cfg_attr(feature = "serde", serde(default))]
    pub mode: Option<BatchMode>,
    pub jobs: Vec<BatchJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Reports validation failures for batch parsing or compilation.
pub struct BatchValidationError(String);

impl BatchValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for BatchValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BatchValidationError {}

impl BatchSpec {
    /// Validates and normalizes the parsed batch spec.
    ///
    /// # Errors
    ///
    /// Returns [`BatchValidationError`] if the spec is invalid.
    pub fn validate_and_normalize(&mut self) -> Result<(), BatchValidationError> {
        if self.api_version.trim().is_empty() {
            return Err(BatchValidationError::new("api_version is required"));
        }
        match self.kind.as_str() {
            "batch" => {}
            "yolo" => self.kind = "batch".to_string(),
            other => {
                return Err(BatchValidationError::new(format!(
                    "kind must be 'batch' or 'yolo', got '{other}'"
                )))
            }
        }
        if self.worker.template.trim().is_empty() {
            return Err(BatchValidationError::new("worker.template is required"));
        }
        if self.jobs.is_empty() {
            return Err(BatchValidationError::new("jobs must not be empty"));
        }
        for (index, job) in self.jobs.iter().enumerate() {
            if job.prompt.trim().is_empty() {
                return Err(BatchValidationError::new(format!(
                    "jobs[{index}].prompt is required"
                )));
            }
        }
        if let Some(mode) = &self.mode {
            if let Some(parallelism) = mode.parallelism {
                if parallelism == 0 {
                    return Err(BatchValidationError::new(
                        "mode.parallelism must be positive",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Parses a YAML batch spec.
///
/// # Examples
///
/// ```
/// let spec = void_control::batch::parse_batch_yaml(
///     r#"
/// api_version: v1
/// kind: batch
/// worker:
///   template: examples/runtime-templates/warm_agent_basic.yaml
/// jobs:
///   - prompt: Fix failing auth tests
/// "#,
/// )
/// .expect("parse batch");
/// assert_eq!(spec.kind, "batch");
/// ```
///
/// # Errors
///
/// Returns [`BatchValidationError`] if the YAML is invalid or the parsed spec
/// fails validation.
pub fn parse_batch_yaml(yaml: &str) -> Result<BatchSpec, BatchValidationError> {
    let mut spec: BatchSpec = serde_yaml::from_str(yaml)
        .map_err(|err| BatchValidationError::new(format!("invalid batch yaml: {err}")))?;
    spec.validate_and_normalize()?;
    Ok(spec)
}

/// Parses a JSON batch spec.
///
/// # Examples
///
/// ```
/// let spec = void_control::batch::parse_batch_json(
///     r#"{
///   "api_version": "v1",
///   "kind": "yolo",
///   "worker": { "template": "examples/runtime-templates/warm_agent_basic.yaml" },
///   "jobs": [{ "prompt": "Review migration safety" }]
/// }"#,
/// )
/// .expect("parse batch");
/// assert_eq!(spec.kind, "batch");
/// ```
///
/// # Errors
///
/// Returns [`BatchValidationError`] if the JSON is invalid or the parsed spec
/// fails validation.
pub fn parse_batch_json(json: &str) -> Result<BatchSpec, BatchValidationError> {
    let mut spec: BatchSpec = serde_json::from_str(json)
        .map_err(|err| BatchValidationError::new(format!("invalid batch json: {err}")))?;
    spec.validate_and_normalize()?;
    Ok(spec)
}
