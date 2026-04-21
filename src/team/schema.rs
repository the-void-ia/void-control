use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes optional team metadata.
pub struct TeamMetadata {
    #[cfg_attr(feature = "serde", serde(default))]
    pub name: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one named team agent.
pub struct AgentSpec {
    pub name: String,
    pub role: String,
    pub goal: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub template: Option<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one team task.
pub struct TaskSpec {
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub agent: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub depends_on: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes the team process mode.
pub struct ProcessSpec {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub kind: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes a high-level team submission.
pub struct TeamSpec {
    pub api_version: String,
    pub kind: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub metadata: Option<TeamMetadata>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub agents: Vec<AgentSpec>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub tasks: Vec<TaskSpec>,
    pub process: ProcessSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Reports validation failures for team parsing or compilation.
pub struct TeamValidationError(String);

impl TeamValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for TeamValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for TeamValidationError {}

impl TeamSpec {
    /// Validates the parsed team spec.
    ///
    /// # Errors
    ///
    /// Returns [`TeamValidationError`] if the spec is invalid.
    pub fn validate(&self) -> Result<(), TeamValidationError> {
        if self.api_version.trim().is_empty() {
            return Err(TeamValidationError::new("api_version is required"));
        }
        if self.kind != "team" {
            return Err(TeamValidationError::new(format!(
                "kind must be 'team', got '{}'",
                self.kind
            )));
        }
        if self.agents.is_empty() {
            return Err(TeamValidationError::new(
                "team spec must include at least one agent",
            ));
        }
        if self.tasks.is_empty() {
            return Err(TeamValidationError::new(
                "team spec must include at least one task",
            ));
        }

        let mut agent_names = BTreeSet::new();
        for agent in &self.agents {
            if agent.name.trim().is_empty() {
                return Err(TeamValidationError::new("agents[].name is required"));
            }
            if agent.role.trim().is_empty() {
                return Err(TeamValidationError::new(format!(
                    "agents['{}'].role is required",
                    agent.name
                )));
            }
            if agent.goal.trim().is_empty() {
                return Err(TeamValidationError::new(format!(
                    "agents['{}'].goal is required",
                    agent.name
                )));
            }
            if !agent_names.insert(agent.name.clone()) {
                return Err(TeamValidationError::new(format!(
                    "duplicate agent name '{}'",
                    agent.name
                )));
            }
        }

        match self.process.kind.as_str() {
            "sequential" | "parallel" | "lead_worker" => {}
            other => {
                return Err(TeamValidationError::new(format!(
                    "process.type must be one of sequential, parallel, lead_worker; got '{other}'"
                )))
            }
        }

        let single_agent_name = if self.agents.len() == 1 {
            Some(self.agents[0].name.as_str())
        } else {
            None
        };
        for task in &self.tasks {
            if task.name.trim().is_empty() {
                return Err(TeamValidationError::new("tasks[].name is required"));
            }
            if task.description.trim().is_empty() {
                return Err(TeamValidationError::new(format!(
                    "tasks['{}'].description is required",
                    task.name
                )));
            }
            let agent_name = match task.agent.as_deref() {
                Some(agent_name) => agent_name,
                None => {
                    let Some(agent_name) = single_agent_name else {
                        return Err(TeamValidationError::new(format!(
                            "tasks['{}'].agent is required when multiple agents are defined",
                            task.name
                        )));
                    };
                    agent_name
                }
            };
            if !agent_names.contains(agent_name) {
                return Err(TeamValidationError::new(format!(
                    "tasks['{}'].agent references unknown agent '{}'",
                    task.name, agent_name
                )));
            }
        }

        Ok(())
    }
}

/// Parses a YAML team spec.
///
/// # Errors
///
/// Returns [`TeamValidationError`] if the YAML is invalid or the parsed spec
/// fails validation.
pub fn parse_team_yaml(yaml: &str) -> Result<TeamSpec, TeamValidationError> {
    let spec: TeamSpec = serde_yaml::from_str(yaml)
        .map_err(|err| TeamValidationError::new(format!("invalid team yaml: {err}")))?;
    spec.validate()?;
    Ok(spec)
}

/// Parses a JSON team spec.
///
/// # Errors
///
/// Returns [`TeamValidationError`] if the JSON is invalid or the parsed spec
/// fails validation.
pub fn parse_team_json(json: &str) -> Result<TeamSpec, TeamValidationError> {
    let spec: TeamSpec = serde_json::from_str(json)
        .map_err(|err| TeamValidationError::new(format!("invalid team json: {err}")))?;
    spec.validate()?;
    Ok(spec)
}
