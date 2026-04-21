use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::orchestration::{ExecutionSpec, GlobalConfig};

use super::{ControlTemplate, TemplateMetadata, TemplateValidationError};

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTemplate {
    pub template: TemplateMetadata,
    pub normalized_inputs: BTreeMap<String, Value>,
    pub execution_spec: ExecutionSpec,
}

pub fn compile_template(
    template: &ControlTemplate,
    inputs: &Value,
) -> Result<CompiledTemplate, TemplateValidationError> {
    let input_object = inputs
        .as_object()
        .ok_or_else(|| TemplateValidationError::new("template inputs must be a JSON object"))?;
    let normalized_inputs = normalize_inputs(template, input_object)?;
    let mut execution_spec = template.defaults.execution_spec.clone();
    execution_spec.workflow.template = template.defaults.workflow_template.clone();

    for binding in &template.compile.bindings {
        let Some(value) = normalized_inputs.get(&binding.input) else {
            let field = template.inputs.get(&binding.input).ok_or_else(|| {
                TemplateValidationError::new(format!(
                    "binding references unknown normalized input '{}'",
                    binding.input
                ))
            })?;
            if field.required || field.default.is_some() {
                return Err(TemplateValidationError::new(format!(
                    "binding references missing normalized input '{}'",
                    binding.input
                )));
            }
            continue;
        };
        apply_binding(&mut execution_spec, &binding.target, value)?;
    }

    validate_execution_kind_shape(template, &execution_spec)?;
    execution_spec
        .validate(&GlobalConfig {
            max_concurrent_child_runs: 20,
        })
        .map_err(|err| {
            TemplateValidationError::new(format!("compiled execution spec is invalid: {err}"))
        })?;

    Ok(CompiledTemplate {
        template: template.template.clone(),
        normalized_inputs,
        execution_spec,
    })
}

fn normalize_inputs(
    template: &ControlTemplate,
    inputs: &Map<String, Value>,
) -> Result<BTreeMap<String, Value>, TemplateValidationError> {
    for key in inputs.keys() {
        if !template.inputs.contains_key(key) {
            return Err(TemplateValidationError::new(format!(
                "unknown input '{}'",
                key
            )));
        }
    }

    let mut normalized = BTreeMap::new();
    for (name, field) in &template.inputs {
        let value = match inputs.get(name) {
            Some(value) => value.clone(),
            None => match &field.default {
                Some(value) => value.clone(),
                None if field.required => {
                    return Err(TemplateValidationError::new(format!(
                        "missing required input '{}'",
                        name
                    )))
                }
                None => continue,
            },
        };

        validate_input_value(name, field, &value)?;
        normalized.insert(name.clone(), value);
    }
    Ok(normalized)
}

fn validate_input_value(
    name: &str,
    field: &super::InputField,
    value: &Value,
) -> Result<(), TemplateValidationError> {
    match field.field_type.as_str() {
        "string" => {
            if !value.is_string() {
                return Err(TemplateValidationError::new(format!(
                    "input '{}' must be a string",
                    name
                )));
            }
        }
        "enum" => {
            let Some(raw) = value.as_str() else {
                return Err(TemplateValidationError::new(format!(
                    "input '{}' must be a string",
                    name
                )));
            };
            let values = field.values.as_ref().ok_or_else(|| {
                TemplateValidationError::new(format!(
                    "input '{}' enum values are not configured",
                    name
                ))
            })?;
            if !values.iter().any(|candidate| candidate == raw) {
                return Err(TemplateValidationError::new(format!(
                    "input '{}' must be one of [{}]",
                    name,
                    values.join(", ")
                )));
            }
        }
        "integer" => {
            let Some(number) = value.as_i64().or_else(|| value.as_u64().map(|n| n as i64)) else {
                return Err(TemplateValidationError::new(format!(
                    "input '{}' must be an integer",
                    name
                )));
            };
            validate_numeric_range(name, field, number as f64)?;
        }
        "number" => {
            let Some(number) = value.as_f64() else {
                return Err(TemplateValidationError::new(format!(
                    "input '{}' must be a number",
                    name
                )));
            };
            validate_numeric_range(name, field, number)?;
        }
        "boolean" => {
            if !value.is_boolean() {
                return Err(TemplateValidationError::new(format!(
                    "input '{}' must be a boolean",
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
    Ok(())
}

fn validate_numeric_range(
    name: &str,
    field: &super::InputField,
    value: f64,
) -> Result<(), TemplateValidationError> {
    if let Some(min) = field.min {
        if value < min {
            return Err(TemplateValidationError::new(format!(
                "input '{}' must be >= {}",
                name, min
            )));
        }
    }
    if let Some(max) = field.max {
        if value > max {
            return Err(TemplateValidationError::new(format!(
                "input '{}' must be <= {}",
                name, max
            )));
        }
    }
    Ok(())
}

fn apply_binding(
    execution_spec: &mut ExecutionSpec,
    target: &str,
    value: &Value,
) -> Result<(), TemplateValidationError> {
    if let Some(path) = target.strip_prefix("execution_spec.") {
        let mut spec_json = serde_json::to_value(&*execution_spec).map_err(|err| {
            TemplateValidationError::new(format!("failed to serialize execution spec: {err}"))
        })?;
        set_json_path(&mut spec_json, path, value.clone())?;
        *execution_spec = serde_json::from_value(spec_json).map_err(|err| {
            TemplateValidationError::new(format!(
                "failed to deserialize bound execution spec: {err}"
            ))
        })?;
        return Ok(());
    }

    if let Some((index, key)) = parse_explicit_override_target(target)? {
        let raw = value_to_string(value)?;
        let proposal = execution_spec
            .variation
            .explicit
            .get_mut(index)
            .ok_or_else(|| {
                TemplateValidationError::new(format!(
                    "variation.explicit[{index}] is required for override bindings"
                ))
            })?;
        proposal.overrides.insert(key.to_string(), raw);
        return Ok(());
    }

    Err(TemplateValidationError::new(format!(
        "unsupported binding target '{}'",
        target
    )))
}

fn parse_explicit_override_target(
    target: &str,
) -> Result<Option<(usize, &str)>, TemplateValidationError> {
    let Some(rest) = target.strip_prefix("variation.explicit[") else {
        return Ok(None);
    };
    let Some(close) = rest.find(']') else {
        return Err(TemplateValidationError::new(format!(
            "unsupported binding target '{}'",
            target
        )));
    };
    let index = rest[..close].parse::<usize>().map_err(|_| {
        TemplateValidationError::new(format!("unsupported binding target '{}'", target))
    })?;
    let suffix = &rest[close + 1..];
    let Some(key) = suffix.strip_prefix(".overrides.") else {
        return Err(TemplateValidationError::new(format!(
            "unsupported binding target '{}'",
            target
        )));
    };
    if key.is_empty() {
        return Err(TemplateValidationError::new(format!(
            "unsupported binding target '{}'",
            target
        )));
    }
    Ok(Some((index, key)))
}

fn set_json_path(
    root: &mut Value,
    path: &str,
    value: Value,
) -> Result<(), TemplateValidationError> {
    let tokens = parse_path_tokens(path)?;
    let mut current = root;

    for token in &tokens[..tokens.len().saturating_sub(1)] {
        current = match token {
            PathToken::Field(name) => current.get_mut(name).ok_or_else(|| {
                TemplateValidationError::new(format!("unknown execution_spec field '{}'", name))
            })?,
            PathToken::Index(index) => current.get_mut(*index).ok_or_else(|| {
                TemplateValidationError::new(format!("missing execution_spec index [{}]", index))
            })?,
        };
    }

    match tokens.last() {
        Some(PathToken::Field(name)) => {
            let object = current.as_object_mut().ok_or_else(|| {
                TemplateValidationError::new(format!(
                    "execution_spec target parent for '{}' is not an object",
                    name
                ))
            })?;
            object.insert(name.clone(), value);
            Ok(())
        }
        Some(PathToken::Index(index)) => {
            let array = current.as_array_mut().ok_or_else(|| {
                TemplateValidationError::new(format!(
                    "execution_spec target parent for index [{}] is not an array",
                    index
                ))
            })?;
            if *index >= array.len() {
                return Err(TemplateValidationError::new(format!(
                    "missing execution_spec index [{}]",
                    index
                )));
            }
            array[*index] = value;
            Ok(())
        }
        None => Err(TemplateValidationError::new(
            "binding path must not be empty",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathToken {
    Field(String),
    Index(usize),
}

fn parse_path_tokens(path: &str) -> Result<Vec<PathToken>, TemplateValidationError> {
    let mut tokens = Vec::new();
    for part in path.split('.') {
        let mut rest = part;
        loop {
            let Some(open) = rest.find('[') else {
                if !rest.is_empty() {
                    tokens.push(PathToken::Field(rest.to_string()));
                }
                break;
            };

            if open > 0 {
                tokens.push(PathToken::Field(rest[..open].to_string()));
            }

            let close = rest[open + 1..].find(']').ok_or_else(|| {
                TemplateValidationError::new(format!("invalid binding path '{}'", path))
            })? + open
                + 1;
            let index = rest[open + 1..close].parse::<usize>().map_err(|_| {
                TemplateValidationError::new(format!("invalid binding path '{}'", path))
            })?;
            tokens.push(PathToken::Index(index));
            rest = &rest[close + 1..];
            if rest.is_empty() {
                break;
            }
        }
    }

    if tokens.is_empty() {
        return Err(TemplateValidationError::new(
            "binding path must not be empty",
        ));
    }
    Ok(tokens)
}

fn value_to_string(value: &Value) -> Result<String, TemplateValidationError> {
    if let Some(raw) = value.as_str() {
        return Ok(raw.to_string());
    }
    if let Some(raw) = value.as_bool() {
        return Ok(raw.to_string());
    }
    if let Some(raw) = value.as_i64() {
        return Ok(raw.to_string());
    }
    if let Some(raw) = value.as_u64() {
        return Ok(raw.to_string());
    }
    if let Some(raw) = value.as_f64() {
        return Ok(raw.to_string());
    }
    Err(TemplateValidationError::new(
        "override bindings only support scalar input values",
    ))
}

fn validate_execution_kind_shape(
    template: &ControlTemplate,
    execution_spec: &ExecutionSpec,
) -> Result<(), TemplateValidationError> {
    if matches!(
        template.template.execution_kind.as_str(),
        "single_agent" | "warm_agent"
    ) {
        if execution_spec.variation.source != "explicit" {
            return Err(TemplateValidationError::new(format!(
                "execution_kind '{}' requires variation.source=explicit",
                template.template.execution_kind
            )));
        }
        if execution_spec.variation.explicit.len() != 1 {
            return Err(TemplateValidationError::new(format!(
                "execution_kind '{}' requires exactly one explicit variation proposal",
                template.template.execution_kind
            )));
        }
    }
    Ok(())
}
