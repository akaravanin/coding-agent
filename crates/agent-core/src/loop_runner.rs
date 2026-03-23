use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn};

use agent_protocol::{
    AgentError, AgentEvent, ContentBlock, Message, MessageRole,
    ToolCall, ToolResult,
};
use agent_llm::{LlmProvider, LlmRequest};
use agent_llm::request::StopReason;
use agent_tools::{ApprovalCallback, ToolRegistry};
use agent_tools::sandbox::Sandbox;
use agent_context::session_context::SessionContext;

use crate::dispatch::dispatch_tool_call;

/// Drives the agent's agentic loop:
///   1. Build LLM request from session history
///   2. Get response (streaming)
///   3. Emit events for text/tool deltas
///   4. If tool calls present, dispatch each with approval, collect results
///   5. Append tool results to history and loop
///   6. Stop when stop_reason == EndTurn or max_iterations reached
pub struct LoopRunner {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    approval: Arc<dyn ApprovalCallback>,
    sandbox: Sandbox,
    events: broadcast::Sender<AgentEvent>,
}

impl LoopRunner {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<ToolRegistry>,
        approval: Arc<dyn ApprovalCallback>,
        sandbox: Sandbox,
        events: broadcast::Sender<AgentEvent>,
    ) -> Self {
        Self { provider, tools, approval, sandbox, events }
    }

    pub async fn run(&self, mut ctx: SessionContext) -> Result<(), AgentError> {
        loop {
            if ctx.has_reached_limit() {
                warn!("Agent reached max iterations ({})", ctx.config.max_iterations);
                let _ = self.events.send(AgentEvent::Warning {
                    message: format!("Stopped after {} iterations", ctx.config.max_iterations),
                });
                break;
            }

            ctx.iteration += 1;
            debug!("Loop iteration {}", ctx.iteration);

            let _ = self.events.send(AgentEvent::ThinkingStarted);

            let request = LlmRequest::builder(String::new()) // provider uses its default model
                .messages(ctx.history.clone())
                .system(ctx.system_prompt())
                .tools(self.tools.schemas())
                .stream()
                .build();

            let response = self.provider.complete(request).await.map_err(|e| {
                let _ = self.events.send(AgentEvent::session_error(&e));
                e
            })?;

            let _ = self.events.send(AgentEvent::ThinkingDone);

            // Emit text delta for any text content
            let text = response.message.text();
            if !text.is_empty() {
                let _ = self.events.send(AgentEvent::MessageDelta { text });
            }

            // Collect tool calls from this response
            let tool_calls: Vec<ToolCall> = response.message.content.iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse(c) => Some(c.clone()),
                    _ => None,
                })
                .collect();

            // Add assistant message to history
            ctx.push_message(response.message.clone());

            let _ = self.events.send(AgentEvent::MessageComplete {
                message: response.message.clone(),
            });

            if tool_calls.is_empty() || response.stop_reason == StopReason::EndTurn {
                // No more tool calls — we're done
                let _ = self.events.send(AgentEvent::SessionComplete);
                break;
            }

            // Dispatch all tool calls, collect results
            let mut tool_results: Vec<ToolResult> = vec![];
            for call in tool_calls {
                let result = dispatch_tool_call(
                    call,
                    &self.tools,
                    &self.approval,
                    &self.sandbox,
                    &self.events,
                ).await;
                tool_results.push(result);
            }

            // Build a tool-result message and append to history
            let result_content: Vec<ContentBlock> = tool_results
                .into_iter()
                .map(ContentBlock::ToolResult)
                .collect();

            ctx.push_message(Message {
                role: MessageRole::Tool,
                content: result_content,
            });
        }

        Ok(())
    }
}
