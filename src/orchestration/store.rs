mod fs;

use std::io;

use super::types::{ExecutionCandidate, ExecutionSnapshot};

pub use fs::FsExecutionStore;

pub trait ExecutionStore {
    fn load_execution(&self, execution_id: &str) -> io::Result<ExecutionSnapshot>;
    fn list_active_execution_ids(&self) -> io::Result<Vec<String>>;
    fn load_candidates(&self, execution_id: &str) -> io::Result<Vec<ExecutionCandidate>>;
}
