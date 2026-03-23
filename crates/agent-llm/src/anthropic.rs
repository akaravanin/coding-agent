use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::debug;

use agent_protocol::{AgentError, Message, MessageRole, ContentBlock, ToolCall};
use crate::provider::{LlmProvider, StreamResult};
use crate::request::{LlmRequest, LlmResponse, StopReason, TokenUsage};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    default_model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            default_model: "claude-sonnet-4-6".into(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    fn resolve_model<'a>(&'a self, req: &'a LlmRequest) -> &'a str {
        if req.model.is_empty() { &self.default_model } else { &req.model }
    }

    fn build_body(&self, req: &LlmRequest) -> Value {
        let messages: Vec<Value> = req.messages.iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| message_to_anthropic(m))
            .collect();

        let mut body = json!({
            "model": self.resolve_model(req),
            "max_tokens": req.max_tokens,
            "messages": messages,
        });

        if let Some(sys) = &req.system {
            body["system"] = Value::String(sys.clone());
        }
        if !req.tools.is_empty() {
            body["tools"] = serde_json::to_value(&req.tools).unwrap_or_default();
        }
        if let Some(t) = req.temperature {
            body["temperature"] = Value::from(t);
        }
        body
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, AgentError> {
        let mut body = self.build_body(&request);
        body["stream"] = Value::Bool(false);

        debug!(model = %self.resolve_model(&request), "anthropic complete");

        let resp = self.client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Llm(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AgentError::Llm(format!("HTTP {status}: {text}")));
        }

        let raw: Value = resp.json().await
            .map_err(|e| AgentError::Llm(e.to_string()))?;

        parse_anthropic_response(raw)
    }

    async fn stream(&self, request: LlmRequest) -> Result<StreamResult, AgentError> {
        // TODO: implement SSE streaming
        // For now, fall back to complete() and yield a single event sequence.
        use futures::stream;
        use crate::stream::LlmStreamEvent;

        let response = self.complete(request).await?;
        let text = response.message.text();
        let usage = response.usage;
        let stop_reason = response.stop_reason;

        let events: Vec<Result<LlmStreamEvent, AgentError>> = vec![
            Ok(LlmStreamEvent::TextDelta { text }),
            Ok(LlmStreamEvent::StreamEnd { stop_reason, usage }),
        ];

        Ok(Box::pin(stream::iter(events)))
    }
}

// --- Parsing helpers ---

fn parse_anthropic_response(raw: Value) -> Result<LlmResponse, AgentError> {
    let stop_reason = match raw["stop_reason"].as_str() {
        Some("end_turn") => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    };

    let usage = TokenUsage {
        input_tokens: raw["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
        output_tokens: raw["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
    };

    let mut content: Vec<ContentBlock> = vec![];
    if let Some(blocks) = raw["content"].as_array() {
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    content.push(ContentBlock::Text { text });
                }
                Some("tool_use") => {
                    let call = ToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        input: block["input"].clone(),
                    };
                    content.push(ContentBlock::ToolUse(call));
                }
                _ => {}
            }
        }
    }

    Ok(LlmResponse {
        message: Message { role: MessageRole::Assistant, content },
        stop_reason,
        usage,
    })
}

fn message_to_anthropic(msg: &Message) -> Value {
    let role = match msg.role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        _ => "user",
    };

    let content: Vec<Value> = msg.content.iter().map(|block| match block {
        ContentBlock::Text { text } => json!({ "type": "text", "text": text }),
        ContentBlock::ToolUse(call) => json!({
            "type": "tool_use",
            "id": call.id,
            "name": call.name,
            "input": call.input,
        }),
        ContentBlock::ToolResult(result) => json!({
            "type": "tool_result",
            "tool_use_id": result.call_id,
            "content": result.output.to_string(),
        }),
    }).collect();

    json!({ "role": role, "content": content })
}
