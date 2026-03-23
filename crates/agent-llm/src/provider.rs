use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use agent_protocol::AgentError;
use crate::request::{LlmRequest, LlmResponse};
use crate::stream::LlmStreamEvent;

pub type StreamResult = Pin<Box<dyn Stream<Item = Result<LlmStreamEvent, AgentError>> + Send>>;

/// Trait all LLM backends must implement.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name for logging (e.g. "anthropic", "gemini").
    fn name(&self) -> &str;

    /// Non-streaming completion. Implementations may use streaming internally.
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, AgentError>;

    /// Streaming completion. Yields `LlmStreamEvent`s until `StreamEnd`.
    async fn stream(&self, request: LlmRequest) -> Result<StreamResult, AgentError>;
}
