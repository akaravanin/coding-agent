use std::sync::Arc;
use tokio::sync::broadcast;

use agent_protocol::{AgentError, AgentEvent, SessionConfig};
use agent_llm::LlmProvider;
use agent_tools::{ApprovalCallback, AutoApprove, ToolRegistry};
use agent_tools::sandbox::Sandbox;
use agent_tools::{ShellTool, FileReadTool, FileWriteTool, FileSearchTool, MemoryTool, GitTool};

use crate::agent::Agent;

/// Fluent builder for `Agent`.
pub struct AgentBuilder {
    provider: Option<Arc<dyn LlmProvider>>,
    tools: ToolRegistry,
    approval: Arc<dyn ApprovalCallback>,
    sandbox: Option<Sandbox>,
    config: Option<SessionConfig>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            provider: None,
            tools: default_tool_registry(),
            approval: Arc::new(AutoApprove),
            sandbox: None,
            config: None,
        }
    }

    pub fn provider(mut self, provider: impl LlmProvider + 'static) -> Self {
        self.provider = Some(Arc::new(provider));
        self
    }

    pub fn approval(mut self, callback: impl ApprovalCallback + 'static) -> Self {
        self.approval = Arc::new(callback);
        self
    }

    pub fn tools(mut self, registry: ToolRegistry) -> Self {
        self.tools = registry;
        self
    }

    pub fn sandbox(mut self, sandbox: Sandbox) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    pub fn config(mut self, config: SessionConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn build(self) -> Result<Agent, AgentError> {
        let provider = self.provider.ok_or_else(|| {
            AgentError::Config("No LLM provider configured. Call .provider(...)".into())
        })?;

        let config = self.config.ok_or_else(|| {
            AgentError::Config("No session config. Call .config(SessionConfig::new(...))".into())
        })?;

        let sandbox = self.sandbox.unwrap_or_else(|| {
            Sandbox::new(&config.workspace_root)
        });

        let (events, _) = broadcast::channel(256);

        Ok(Agent {
            provider,
            tools: Arc::new(self.tools),
            approval: self.approval,
            sandbox,
            config,
            events,
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self { Self::new() }
}

fn default_tool_registry() -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register(FileReadTool)
     .register(FileWriteTool)
     .register(FileSearchTool)
     .register(MemoryTool)
     .register(ShellTool)
     .register(GitTool);
    r
}
