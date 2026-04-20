use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::orchestration::ExecutionSpec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ControlTemplate {
    pub api_version: String,
    pub kind: String,
    pub template: TemplateMetadata,
    pub inputs: BTreeMap<String, InputField>,
    pub defaults: TemplateDefaults,
    pub compile: TemplateCompile,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateMetadata {
    pub id: String,
    pub name: String,
    pub execution_kind: String,
    pub description: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct InputField {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub field_type: String,
    pub required: bool,
    pub description: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub default: Option<serde_json::Value>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub values: Option<Vec<String>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub min: Option<f64>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub max: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDefaults {
    pub workflow_template: String,
    pub execution_spec: ExecutionSpec,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCompile {
    pub bindings: Vec<CompileBinding>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileBinding {
    pub input: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateValidationError(String);

impl TemplateValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for TemplateValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for TemplateValidationError {}

impl ControlTemplate {
    pub fn validate(&self) -> Result<(), TemplateValidationError> {
        if self.api_version.trim().is_empty() {
            return Err(TemplateValidationError::new("api_version is required"));
        }
        if self.kind != "control_template" {
            return Err(TemplateValidationError::new(format!(
                "unsupported template kind '{}'",
                self.kind
            )));
        }
        if !matches!(
            self.template.execution_kind.as_str(),
            "single_agent" | "warm_agent"
        ) {
            return Err(TemplateValidationError::new(format!(
                "unsupported execution_kind '{}'",
                self.template.execution_kind
            )));
        }
        if self.defaults.workflow_template.trim().is_empty() {
            return Err(TemplateValidationError::new(
                "defaults.workflow_template is required",
            ));
        }
        if self.compile.bindings.is_empty() {
            return Err(TemplateValidationError::new(
                "compile.bindings must not be empty",
            ));
        }
        for (name, field) in &self.inputs {
            match field.field_type.as_str() {
                "string" | "integer" | "number" | "boolean" => {}
                "enum" => {
                    let values = field.values.as_ref().ok_or_else(|| {
                        TemplateValidationError::new(format!(
                            "input '{}' of type enum must define values",
                            name
                        ))
                    })?;
                    if values.is_empty() {
                        return Err(TemplateValidationError::new(format!(
                            "input '{}' enum values must not be empty",
                            name
                        )));
                    }
                }
                other => {
                    return Err(TemplateValidationError::new(format!(
                        "unsupported input type '{}' for '{}'",
                        other, name
                    )))
                }
            }
        }
        Ok(())
    }
}
