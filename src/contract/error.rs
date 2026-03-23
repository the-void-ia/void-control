#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractErrorCode {
    InvalidSpec,
    InvalidPolicy,
    NotFound,
    AlreadyTerminal,
    ResourceLimitExceeded,
    StructuredOutputMissing,
    StructuredOutputMalformed,
    ArtifactNotFound,
    ArtifactPublicationIncomplete,
    ArtifactStoreUnavailable,
    RetrievalTimeout,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    pub code: ContractErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl ContractError {
    pub fn new(code: ContractErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}
