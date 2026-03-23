use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM provider error: {0}")]
    Llm(String),

    #[error("Tool execution error: {tool} — {message}")]
    Tool { tool: String, message: String },

    #[error("Tool call denied by approval callback: {tool}")]
    ToolDenied { tool: String },

    #[error("Context error: {0}")]
    Context(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Configuration error: {0}")]
    Config(String),
}
