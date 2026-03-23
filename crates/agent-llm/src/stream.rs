use serde::{Deserialize, Serialize};
use agent_protocol::{ToolCall, message::ContentBlock};
use crate::request::{StopReason, TokenUsage};

/// Individual events in a streaming LLM response.
/// Consumers accumulate these to build a full `LlmResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LlmStreamEvent {
    /// A text token delta.
    TextDelta { text: String },

    /// The LLM started a tool call block.
    ToolCallStarted { call: ToolCall },

    /// Input JSON for the current tool call is streaming in.
    ToolCallDelta { call_id: String, json_delta: String },

    /// Tool call input is complete and ready to dispatch.
    ToolCallReady { call: ToolCall },

    /// Stream finished; contains final stop reason and token counts.
    StreamEnd {
        stop_reason: StopReason,
        usage: TokenUsage,
    },

    /// Provider-level error mid-stream.
    StreamError { message: String },
}
