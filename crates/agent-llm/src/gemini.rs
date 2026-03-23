use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::debug;

use agent_protocol::{AgentError, ContentBlock, Message, MessageRole, ToolCall};
use crate::provider::{LlmProvider, StreamResult};
use crate::request::{LlmRequest, LlmResponse, StopReason, TokenUsage};

const GEMINI_API_BASE: &str =
    "https://generativelanguage.googleapis.com/v1beta/models";

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    default_model: String,
}

impl GeminiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            default_model: "gemini-2.0-flash".into(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    fn resolve_model<'a>(&'a self, req: &'a LlmRequest) -> &'a str {
        if req.model.is_empty() { &self.default_model } else { &req.model }
    }

    fn endpoint(&self, model: &str) -> String {
        format!(
            "{}/{}/{}:generateContent?key={}",
            GEMINI_API_BASE, model, "", self.api_key
        )
        // Gemini URL is: /v1beta/models/{model}:generateContent?key=...
        .replace("/:generateContent", ":generateContent")
    }

    fn url(&self, model: &str) -> String {
        format!(
            "{}/{}:generateContent?key={}",
            GEMINI_API_BASE, model, self.api_key
        )
    }

    fn build_body(&self, req: &LlmRequest) -> Value {
        let contents = messages_to_gemini(&req.messages);

        let mut body = json!({ "contents": contents });

        // System prompt
        if let Some(sys) = &req.system {
            body["systemInstruction"] = json!({
                "parts": [{ "text": sys }]
            });
        }

        // Tools — Gemini uses functionDeclarations
        if !req.tools.is_empty() {
            let declarations: Vec<Value> = req.tools.iter().map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                })
            }).collect();
            body["tools"] = json!([{ "functionDeclarations": declarations }]);
        }

        // Generation config
        let mut gen_config = json!({ "maxOutputTokens": req.max_tokens });
        if let Some(t) = req.temperature {
            gen_config["temperature"] = Value::from(t);
        }
        body["generationConfig"] = gen_config;

        body
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str { "gemini" }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, AgentError> {
        let model = self.resolve_model(&request).to_string();
        let url = self.url(&model);
        let body = self.build_body(&request);

        debug!(model = %model, "gemini complete");

        let resp = self.client
            .post(&url)
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

        parse_gemini_response(raw)
    }

    async fn stream(&self, request: LlmRequest) -> Result<StreamResult, AgentError> {
        // Fall back to complete() — streaming SSE for Gemini is a future task
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

// --- Parsing ---

fn parse_gemini_response(raw: Value) -> Result<LlmResponse, AgentError> {
    let candidate = raw["candidates"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| AgentError::Llm("Gemini: no candidates in response".into()))?;

    let stop_reason = match candidate["finishReason"].as_str() {
        Some("STOP")              => StopReason::EndTurn,
        Some("FUNCTION_CALLING") => StopReason::ToolUse,
        Some("MAX_TOKENS")       => StopReason::MaxTokens,
        _                        => StopReason::EndTurn,
    };

    let usage = TokenUsage {
        input_tokens:  raw["usageMetadata"]["promptTokenCount"].as_u64().unwrap_or(0) as u32,
        output_tokens: raw["usageMetadata"]["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
    };

    let mut content: Vec<ContentBlock> = vec![];
    let parts = candidate["content"]["parts"].as_array();

    if let Some(parts) = parts {
        for (i, part) in parts.iter().enumerate() {
            if let Some(text) = part["text"].as_str() {
                content.push(ContentBlock::Text { text: text.to_string() });
            } else if let Some(fc) = part.get("functionCall") {
                // Gemini doesn't provide tool call IDs — generate one
                let id = format!("gemini-call-{i}");
                let name = fc["name"].as_str().unwrap_or("").to_string();
                // Gemini uses "args", map to our "input"
                let input = fc["args"].clone();
                content.push(ContentBlock::ToolUse(ToolCall { id, name, input }));
            }
        }
    }

    Ok(LlmResponse {
        message: Message { role: MessageRole::Assistant, content },
        stop_reason,
        usage,
    })
}

// --- Request conversion ---

fn messages_to_gemini(messages: &[Message]) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];

    for msg in messages {
        match msg.role {
            // System is handled separately via systemInstruction
            MessageRole::System => continue,

            MessageRole::User => {
                let parts = content_blocks_to_gemini_parts(&msg.content, false);
                out.push(json!({ "role": "user", "parts": parts }));
            }

            MessageRole::Assistant => {
                let parts = content_blocks_to_gemini_parts(&msg.content, false);
                out.push(json!({ "role": "model", "parts": parts }));
            }

            // Tool results go back as role "user" with functionResponse parts
            MessageRole::Tool => {
                let parts = content_blocks_to_gemini_parts(&msg.content, true);
                if !parts.is_empty() {
                    out.push(json!({ "role": "user", "parts": parts }));
                }
            }
        }
    }

    out
}

fn content_blocks_to_gemini_parts(blocks: &[ContentBlock], tool_results_only: bool) -> Vec<Value> {
    blocks.iter().filter_map(|block| {
        match block {
            ContentBlock::Text { text } if !tool_results_only => {
                Some(json!({ "text": text }))
            }

            ContentBlock::ToolUse(call) if !tool_results_only => {
                Some(json!({
                    "functionCall": {
                        "name": call.name,
                        "args": call.input,
                    }
                }))
            }

            ContentBlock::ToolResult(result) => {
                // Gemini matches by function name, not ID.
                let name = result.tool_name.as_deref()
                    .unwrap_or(result.call_id.as_str());
                Some(json!({
                    "functionResponse": {
                        "name": name,
                        "response": result.output,
                    }
                }))
            }

            _ => None,
        }
    }).collect()
}
