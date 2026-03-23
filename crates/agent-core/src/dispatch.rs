use std::sync::Arc;
use tokio::sync::broadcast;

use agent_protocol::{AgentError, AgentEvent, ToolCall, ToolResult};
use agent_tools::{ApprovalCallback, ApprovalDecision, ToolRegistry};
use agent_tools::sandbox::Sandbox;

/// Handles the approval → execution flow for a single tool call.
/// Emits appropriate events to the broadcast channel.
pub async fn dispatch_tool_call(
    call: ToolCall,
    tools: &ToolRegistry,
    approval: &Arc<dyn ApprovalCallback>,
    sandbox: &Sandbox,
    events: &broadcast::Sender<AgentEvent>,
) -> ToolResult {
    let tool = match tools.get(&call.name) {
        Some(t) => t,
        None => {
            let result = ToolResult::error(
                call.id.clone(),
                format!("Unknown tool: '{}'", call.name),
            );
            let _ = events.send(AgentEvent::ToolCallCompleted { result: result.clone() });
            return result;
        }
    };

    let requires_approval = tool.requires_approval();

    let _ = events.send(AgentEvent::ToolCallRequested {
        call: call.clone(),
        requires_approval,
    });

    // Run approval check if needed
    if requires_approval {
        let decision = approval.request_approval(&call).await;
        match decision {
            ApprovalDecision::Approved => {
                let _ = events.send(AgentEvent::ToolCallApproved { call_id: call.id.clone() });
            }
            ApprovalDecision::Denied => {
                let _ = events.send(AgentEvent::ToolCallDenied { call_id: call.id.clone() });
                let result = ToolResult::denied(call.id.clone(), &call.name);
                let _ = events.send(AgentEvent::ToolCallCompleted { result: result.clone() });
                return result;
            }
        }
    }

    // Execute
    let result = match tool.execute(&call, sandbox).await {
        Ok(r) => r,
        Err(e) => ToolResult::error(call.id.clone(), e.to_string()),
    };

    let _ = events.send(AgentEvent::ToolCallCompleted { result: result.clone() });
    result
}
