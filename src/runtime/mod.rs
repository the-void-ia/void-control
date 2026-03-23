mod mock;
#[cfg(feature = "serde")]
mod void_box;

use crate::contract::{ContractError, RuntimeInspection, StartRequest, StartResult};
use crate::orchestration::{ExecutionRuntime, StructuredOutputResult};

pub use mock::MockRuntime;
#[cfg(feature = "serde")]
pub use void_box::VoidBoxRuntimeClient;

#[cfg(feature = "serde")]
use crate::orchestration::{CandidateSpec, InboxSnapshot};

#[cfg(feature = "serde")]
pub trait ProviderLaunchAdapter {
    fn prepare_launch_request(
        &self,
        request: StartRequest,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> StartRequest;
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
    ) -> StartRequest {
        debug_assert_eq!(candidate.candidate_id, inbox.candidate_id);
        let launch_context = serde_json::to_string(inbox).expect("serialize inbox snapshot");
        StartRequest {
            launch_context: Some(launch_context),
            ..request
        }
    }
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
}
