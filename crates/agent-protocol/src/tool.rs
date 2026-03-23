use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Unique identifier for a tool call within a session.
pub type ToolCallId = String;

/// A request from the LLM to invoke a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultStatus {
    Success,
    Error,
    Denied,
}

/// The outcome of executing (or denying) a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: ToolCallId,
    pub status: ToolResultStatus,
    /// Structured output returned to the LLM.
    pub output: Value,
    /// Human-readable summary for display.
    pub display: Option<String>,
}

impl ToolResult {
    pub fn success(call_id: impl Into<String>, output: Value) -> Self {
        Self {
            call_id: call_id.into(),
            status: ToolResultStatus::Success,
            output,
            display: None,
        }
    }

    pub fn error(call_id: impl Into<String>, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            call_id: call_id.into(),
            status: ToolResultStatus::Error,
            output: Value::String(msg.clone()),
            display: Some(msg),
        }
    }

    pub fn denied(call_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        let msg = format!("Tool '{}' was denied by the user", tool_name.into());
        Self {
            call_id: call_id.into(),
            status: ToolResultStatus::Denied,
            output: Value::String(msg.clone()),
            display: Some(msg),
        }
    }
}

/// JSON Schema description of a tool, sent to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    /// JSON Schema object for the `input` field.
    pub input_schema: Value,
}
