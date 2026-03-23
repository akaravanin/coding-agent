use serde::{Deserialize, Serialize};
use crate::tool::{ToolCall, ToolResult};
use crate::message::Message;
use crate::error::AgentError;

/// Events emitted by the agent over its broadcast channel.
/// Consumers (CLI, UI, tests) subscribe to drive output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent is reasoning / waiting for LLM response.
    ThinkingStarted,
    ThinkingDone,

    /// A token arrived from the streaming LLM response.
    MessageDelta { text: String },

    /// A full assistant message is ready.
    MessageComplete { message: Message },

    /// LLM requested a tool call. `requires_approval` indicates consumer must
    /// call back before execution proceeds.
    ToolCallRequested {
        call: ToolCall,
        requires_approval: bool,
    },

    /// Approval callback returned approval.
    ToolCallApproved { call_id: String },

    /// Approval callback denied the call.
    ToolCallDenied { call_id: String },

    /// Tool finished executing (success or error).
    ToolCallCompleted { result: ToolResult },

    /// Non-fatal warning (e.g., partial tool failure).
    Warning { message: String },

    /// Fatal session-level error.
    SessionError { error: String },

    /// Session completed normally.
    SessionComplete,
}

impl AgentEvent {
    pub fn session_error(err: &AgentError) -> Self {
        Self::SessionError { error: err.to_string() }
    }
}
