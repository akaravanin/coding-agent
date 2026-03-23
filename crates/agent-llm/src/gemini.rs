use async_trait::async_trait;
use agent_protocol::AgentError;
use crate::provider::{LlmProvider, StreamResult};
use crate::request::{LlmRequest, LlmResponse};

/// Placeholder — Gemini implementation to be added.
pub struct GeminiProvider {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    default_model: String,
}

impl GeminiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: "gemini-2.0-flash".into(),
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str { "gemini" }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, AgentError> {
        Err(AgentError::Llm("Gemini provider not yet implemented".into()))
    }

    async fn stream(&self, _request: LlmRequest) -> Result<StreamResult, AgentError> {
        Err(AgentError::Llm("Gemini provider not yet implemented".into()))
    }
}
