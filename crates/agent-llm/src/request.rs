use serde::{Deserialize, Serialize};
use agent_protocol::{Message, ToolSchema};

/// A request sent to an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub system: Option<String>,
    pub tools: Vec<ToolSchema>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stream: bool,
}

impl LlmRequest {
    pub fn builder(model: impl Into<String>) -> LlmRequestBuilder {
        LlmRequestBuilder::new(model)
    }
}

/// A complete (non-streaming) response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub message: Message,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// --- Builder ---

pub struct LlmRequestBuilder {
    model: String,
    messages: Vec<Message>,
    system: Option<String>,
    tools: Vec<ToolSchema>,
    max_tokens: u32,
    temperature: Option<f32>,
    stream: bool,
}

impl LlmRequestBuilder {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: vec![],
            system: None,
            tools: vec![],
            max_tokens: 8192,
            temperature: None,
            stream: false,
        }
    }

    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn tools(mut self, tools: Vec<ToolSchema>) -> Self {
        self.tools = tools;
        self
    }

    pub fn max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = Some(t);
        self
    }

    pub fn stream(mut self) -> Self {
        self.stream = true;
        self
    }

    pub fn build(self) -> LlmRequest {
        LlmRequest {
            model: self.model,
            messages: self.messages,
            system: self.system,
            tools: self.tools,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: self.stream,
        }
    }
}
