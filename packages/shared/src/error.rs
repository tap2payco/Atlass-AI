use thiserror::Error;

#[derive(Error, Debug)]
pub enum AtlasError {
    #[error("Runtime error: {0}")]
    Runtime(#[from] RuntimeError),

    #[error("Tool execution error: {0}")]
    Tool(#[from] ToolError),

    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),

    #[error("Policy error: {0}")]
    Policy(#[from] PolicyError),

    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigurationError),

    #[error("Inference error: {0}")]
    Inference(#[from] InferenceError),

    #[error("Internal system error: {0}")]
    Internal(String),
}

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Agent failed to initialize: {0}")]
    AgentInitialization(String),
    #[error("Execution timed out after {0} ms")]
    Timeout(u64),
    #[error("Pipeline error: {0}")]
    Pipeline(String),
}

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
}

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Failed to parse AST: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Key not found: {0}")]
    NotFound(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Database error: {0}")]
    Database(String),
}

#[derive(Error, Debug)]
pub enum PolicyError {
    #[error("Action denied: {0}")]
    ActionDenied(String),
    #[error("Resource limit exceeded: {0}")]
    LimitExceeded(String),
}

#[derive(Error, Debug)]
pub enum ConfigurationError {
    #[error("Missing configuration key: {0}")]
    MissingKey(String),
    #[error("Invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },
}

#[derive(Error, Debug)]
pub enum InferenceError {
    #[error("Provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("All providers failed for this request")]
    AllProvidersFailed,

    #[error("Inference timed out after {0} ms")]
    Timeout(u64),

    #[error("Token limit exceeded: limit={limit}, requested={requested}")]
    TokenLimitExceeded { limit: u32, requested: u32 },

    #[error("Circuit breaker open for provider: {0}")]
    CircuitOpen(String),

    #[error("API error from provider '{provider}': {message}")]
    ApiError { provider: String, message: String },

    #[error("Capability not supported by provider: {0}")]
    Unsupported(String),

    #[error("Streaming error: {0}")]
    StreamError(String),
}
