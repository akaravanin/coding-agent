use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::sandbox::Sandbox;

/// Every tool must implement this trait.
#[async_trait]
pub trait Tool: Send + Sync {
    fn schema(&self) -> ToolSchema;

    /// Whether this tool requires approval before execution.
    fn requires_approval(&self) -> bool;

    async fn execute(
        &self,
        call: &ToolCall,
        sandbox: &Sandbox,
    ) -> Result<ToolResult, AgentError>;
}

/// Central registry: maps tool names to implementations.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) -> &mut Self {
        let name = tool.schema().name.clone();
        self.tools.insert(name, Arc::new(tool));
        self
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub fn all(&self) -> impl Iterator<Item = (&str, &Arc<dyn Tool>)> {
        self.tools.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
