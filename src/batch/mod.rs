//! Batch authoring and compilation helpers.

mod compile;
mod schema;

/// Compiles a [`BatchSpec`] into a normal execution plan.
pub use compile::compile_batch_spec;
pub use schema::{
    parse_batch_json, parse_batch_yaml, BatchJob, BatchMetadata, BatchMode, BatchSpec,
    BatchValidationError, BatchWorker,
};

/// Marks the public batch module for compile-time tests.
pub struct BatchModuleMarker;
