//! Team authoring and compilation helpers.

mod compile;
mod schema;

/// Compiles a [`TeamSpec`] into a normal execution plan.
pub use compile::compile_team_spec;
pub use schema::{
    parse_team_json, parse_team_yaml, AgentSpec, ProcessSpec, TaskSpec, TeamMetadata, TeamSpec,
    TeamValidationError,
};

/// Marks the public team module for compile-time tests.
pub struct TeamModuleMarker;
