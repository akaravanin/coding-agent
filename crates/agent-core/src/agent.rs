use std::sync::Arc;
use tokio::sync::broadcast;

use agent_protocol::{AgentError, AgentEvent, Message, SessionConfig};
use agent_llm::LlmProvider;
use agent_tools::{ApprovalCallback, ToolRegistry};
use agent_context::session_context::SessionContext;
use agent_context::workspace::WorkspaceInfo;
use agent_tools::sandbox::Sandbox;
use agent_tools::impls::memory::load_memories;

use crate::loop_runner::LoopRunner;

const EVENT_CHANNEL_CAPACITY: usize = 256;

/// The public handle to a running agent session.
/// Create via `AgentBuilder`. Drive by sending messages; observe via `subscribe()`.
pub struct Agent {
    pub(crate) provider: Arc<dyn LlmProvider>,
    pub(crate) tools: Arc<ToolRegistry>,
    pub(crate) approval: Arc<dyn ApprovalCallback>,
    pub(crate) sandbox: Sandbox,
    pub(crate) config: SessionConfig,
    pub(crate) events: broadcast::Sender<AgentEvent>,
}

impl Agent {
    /// Subscribe to the agent event stream.
    /// Call this before `run()` to avoid missing early events.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.events.subscribe()
    }

    /// Run the agent on a user message.
    /// Returns when the agent produces a final response (no more tool calls) or hits limits.
    pub async fn run(&self, user_message: impl Into<String>) -> Result<(), AgentError> {
        let workspace = WorkspaceInfo::detect(&self.config.workspace_root).await?;

        let mut ctx = SessionContext::new(self.config.clone(), workspace);

        // Load persistent memories from filesystem
        let memories = load_memories(&self.config.workspace_root).await;
        ctx.memories = memories;

        // Prime with the user message
        ctx.push_message(Message::user(user_message.into()));

        let runner = LoopRunner::new(
            self.provider.clone(),
            self.tools.clone(),
            self.approval.clone(),
            self.sandbox.clone(),
            self.events.clone(),
        );

        runner.run(ctx).await
    }
}
