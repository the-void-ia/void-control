use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Describes optional sandbox metadata.
pub struct SandboxMetadata {
    #[cfg_attr(feature = "serde", serde(default))]
    pub name: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub labels: BTreeMap<String, String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one host mount inside a sandbox.
pub struct SandboxMount {
    pub host: String,
    pub guest: String,
    pub mode: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes the runtime configuration for one sandbox.
pub struct SandboxRuntime {
    pub image: String,
    pub cpus: u32,
    pub memory_mb: u32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub network: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub env: BTreeMap<String, String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub mounts: Vec<SandboxMount>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub ports: Vec<u16>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Describes snapshot restore inputs for a sandbox.
pub struct SandboxSnapshot {
    #[cfg_attr(feature = "serde", serde(default))]
    pub restore_from: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Describes lifecycle preferences for a sandbox.
pub struct SandboxLifecycle {
    #[cfg_attr(feature = "serde", serde(default))]
    pub auto_remove: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub detach: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub idle_timeout_secs: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub prewarm: Option<bool>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Describes identity and reuse preferences for a sandbox.
pub struct SandboxIdentity {
    #[cfg_attr(feature = "serde", serde(default))]
    pub reusable: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub pool: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub labels: BTreeMap<String, String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one high-level sandbox submission.
pub struct SandboxSpec {
    pub api_version: String,
    pub kind: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub metadata: Option<SandboxMetadata>,
    pub runtime: SandboxRuntime,
    #[cfg_attr(feature = "serde", serde(default))]
    pub snapshot: Option<SandboxSnapshot>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub lifecycle: Option<SandboxLifecycle>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub identity: Option<SandboxIdentity>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Describes optional snapshot metadata.
pub struct SnapshotMetadata {
    #[cfg_attr(feature = "serde", serde(default))]
    pub name: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub labels: BTreeMap<String, String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes the source sandbox for a snapshot.
pub struct SnapshotSource {
    pub sandbox_id: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes snapshot distribution preferences.
pub struct SnapshotDistribution {
    pub mode: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub targets: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one snapshot resource.
pub struct SnapshotSpec {
    pub api_version: String,
    pub kind: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub metadata: Option<SnapshotMetadata>,
    pub source: SnapshotSource,
    #[cfg_attr(feature = "serde", serde(default))]
    pub distribution: Option<SnapshotDistribution>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes the reusable sandbox shape for a pool.
pub struct SandboxPoolSandboxSpec {
    pub runtime: SandboxRuntime,
    #[cfg_attr(feature = "serde", serde(default))]
    pub snapshot: Option<SandboxSnapshot>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub lifecycle: Option<SandboxLifecycle>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub identity: Option<SandboxIdentity>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes desired warm and maximum pool capacity.
pub struct PoolCapacity {
    pub warm: u32,
    pub max: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one sandbox pool resource.
pub struct SandboxPoolSpec {
    pub api_version: String,
    pub kind: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub metadata: Option<SandboxMetadata>,
    pub sandbox_spec: SandboxPoolSandboxSpec,
    pub capacity: PoolCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Reports validation failures for sandbox resources.
pub struct SandboxValidationError(String);

impl SandboxValidationError {
    /// Creates a new validation error.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for SandboxValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SandboxValidationError {}

impl SandboxSpec {
    /// Validates the parsed sandbox spec.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxValidationError`] if the spec is invalid.
    pub fn validate(&self) -> Result<(), SandboxValidationError> {
        validate_api_version(&self.api_version)?;
        if self.kind != "sandbox" {
            return Err(SandboxValidationError::new(format!(
                "kind must be 'sandbox', got '{}'",
                self.kind
            )));
        }
        validate_runtime(&self.runtime)?;
        validate_snapshot(self.snapshot.as_ref())?;
        validate_lifecycle(self.lifecycle.as_ref())?;
        validate_identity(self.identity.as_ref())?;
        Ok(())
    }
}

impl SnapshotSpec {
    /// Validates the parsed snapshot spec.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxValidationError`] if the spec is invalid.
    pub fn validate(&self) -> Result<(), SandboxValidationError> {
        validate_api_version(&self.api_version)?;
        if self.kind != "snapshot" {
            return Err(SandboxValidationError::new(format!(
                "kind must be 'snapshot', got '{}'",
                self.kind
            )));
        }
        if self.source.sandbox_id.trim().is_empty() {
            return Err(SandboxValidationError::new("source.sandbox_id is required"));
        }
        if let Some(distribution) = &self.distribution {
            match distribution.mode.as_str() {
                "cached" | "copy" => {}
                _ => {
                    return Err(SandboxValidationError::new(
                        "distribution.mode must be one of cached, copy",
                    ))
                }
            }
            if distribution.targets.is_empty() {
                return Err(SandboxValidationError::new(
                    "distribution.targets must not be empty",
                ));
            }
            for target in &distribution.targets {
                if target.trim().is_empty() {
                    return Err(SandboxValidationError::new(
                        "distribution.targets entries must not be empty",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl SandboxPoolSpec {
    /// Validates the parsed sandbox pool spec.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxValidationError`] if the spec is invalid.
    pub fn validate(&self) -> Result<(), SandboxValidationError> {
        validate_api_version(&self.api_version)?;
        if self.kind != "sandbox_pool" {
            return Err(SandboxValidationError::new(format!(
                "kind must be 'sandbox_pool', got '{}'",
                self.kind
            )));
        }
        validate_runtime(&self.sandbox_spec.runtime)?;
        validate_snapshot(self.sandbox_spec.snapshot.as_ref())?;
        validate_lifecycle(self.sandbox_spec.lifecycle.as_ref())?;
        validate_identity(self.sandbox_spec.identity.as_ref())?;
        if self.capacity.max == 0 {
            return Err(SandboxValidationError::new("capacity.max must be positive"));
        }
        if self.capacity.warm > self.capacity.max {
            return Err(SandboxValidationError::new(
                "capacity.warm must not exceed capacity.max",
            ));
        }
        Ok(())
    }
}

fn validate_api_version(api_version: &str) -> Result<(), SandboxValidationError> {
    if api_version.trim().is_empty() {
        return Err(SandboxValidationError::new("api_version is required"));
    }
    Ok(())
}

fn validate_runtime(runtime: &SandboxRuntime) -> Result<(), SandboxValidationError> {
    if runtime.image.trim().is_empty() {
        return Err(SandboxValidationError::new("runtime.image is required"));
    }
    if runtime.cpus == 0 {
        return Err(SandboxValidationError::new("runtime.cpus must be positive"));
    }
    if runtime.memory_mb == 0 {
        return Err(SandboxValidationError::new(
            "runtime.memory_mb must be positive",
        ));
    }
    for mount in &runtime.mounts {
        if mount.host.trim().is_empty() {
            return Err(SandboxValidationError::new(
                "runtime.mounts[].host is required",
            ));
        }
        if mount.guest.trim().is_empty() {
            return Err(SandboxValidationError::new(
                "runtime.mounts[].guest is required",
            ));
        }
        match mount.mode.as_str() {
            "ro" | "rw" => {}
            _ => {
                return Err(SandboxValidationError::new(
                    "runtime.mounts[].mode must be one of ro, rw",
                ))
            }
        }
    }
    for port in &runtime.ports {
        if *port == 0 {
            return Err(SandboxValidationError::new(
                "runtime.ports entries must be positive",
            ));
        }
    }
    Ok(())
}

fn validate_snapshot(snapshot: Option<&SandboxSnapshot>) -> Result<(), SandboxValidationError> {
    let Some(snapshot) = snapshot else {
        return Ok(());
    };
    if let Some(restore_from) = &snapshot.restore_from {
        if restore_from.trim().is_empty() {
            return Err(SandboxValidationError::new(
                "snapshot.restore_from must not be empty",
            ));
        }
    }
    Ok(())
}

fn validate_lifecycle(lifecycle: Option<&SandboxLifecycle>) -> Result<(), SandboxValidationError> {
    let Some(lifecycle) = lifecycle else {
        return Ok(());
    };
    if lifecycle.idle_timeout_secs == Some(0) {
        return Err(SandboxValidationError::new(
            "lifecycle.idle_timeout_secs must be positive",
        ));
    }
    Ok(())
}

fn validate_identity(identity: Option<&SandboxIdentity>) -> Result<(), SandboxValidationError> {
    let Some(identity) = identity else {
        return Ok(());
    };
    if let Some(pool) = &identity.pool {
        if pool.trim().is_empty() {
            return Err(SandboxValidationError::new(
                "identity.pool must not be empty",
            ));
        }
    }
    Ok(())
}

/// Parses a YAML sandbox spec.
///
/// # Errors
///
/// Returns [`SandboxValidationError`] if the YAML is invalid or the parsed spec
/// fails validation.
pub fn parse_sandbox_yaml(yaml: &str) -> Result<SandboxSpec, SandboxValidationError> {
    let spec: SandboxSpec = serde_yaml::from_str(yaml)
        .map_err(|err| SandboxValidationError::new(format!("invalid sandbox yaml: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Parses a JSON sandbox spec.
///
/// # Errors
///
/// Returns [`SandboxValidationError`] if the JSON is invalid or the parsed spec
/// fails validation.
pub fn parse_sandbox_json(json: &str) -> Result<SandboxSpec, SandboxValidationError> {
    let spec: SandboxSpec = serde_json::from_str(json)
        .map_err(|err| SandboxValidationError::new(format!("invalid sandbox json: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Parses a YAML snapshot spec.
///
/// # Errors
///
/// Returns [`SandboxValidationError`] if the YAML is invalid or the parsed spec
/// fails validation.
pub fn parse_snapshot_yaml(yaml: &str) -> Result<SnapshotSpec, SandboxValidationError> {
    let spec: SnapshotSpec = serde_yaml::from_str(yaml)
        .map_err(|err| SandboxValidationError::new(format!("invalid snapshot yaml: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Parses a JSON snapshot spec.
///
/// # Errors
///
/// Returns [`SandboxValidationError`] if the JSON is invalid or the parsed spec
/// fails validation.
pub fn parse_snapshot_json(json: &str) -> Result<SnapshotSpec, SandboxValidationError> {
    let spec: SnapshotSpec = serde_json::from_str(json)
        .map_err(|err| SandboxValidationError::new(format!("invalid snapshot json: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Parses a YAML sandbox pool spec.
///
/// # Errors
///
/// Returns [`SandboxValidationError`] if the YAML is invalid or the parsed spec
/// fails validation.
pub fn parse_pool_yaml(yaml: &str) -> Result<SandboxPoolSpec, SandboxValidationError> {
    let spec: SandboxPoolSpec = serde_yaml::from_str(yaml)
        .map_err(|err| SandboxValidationError::new(format!("invalid sandbox pool yaml: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Parses a JSON sandbox pool spec.
///
/// # Errors
///
/// Returns [`SandboxValidationError`] if the JSON is invalid or the parsed spec
/// fails validation.
pub fn parse_pool_json(json: &str) -> Result<SandboxPoolSpec, SandboxValidationError> {
    let spec: SandboxPoolSpec = serde_json::from_str(json)
        .map_err(|err| SandboxValidationError::new(format!("invalid sandbox pool json: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Marks the public sandbox module for compile-time tests.
pub struct SandboxModuleMarker;
