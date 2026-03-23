use agent_protocol::{Message, SessionConfig};
use crate::workspace::WorkspaceInfo;

/// All mutable state for a single agent session.
/// Lives entirely in memory; no persistence.
pub struct SessionContext {
    pub config: SessionConfig,
    pub workspace: WorkspaceInfo,
    /// The conversation history sent to the LLM each turn.
    pub history: Vec<Message>,
    /// Memory file contents loaded at session start.
    pub memories: Vec<(String, String)>,
    /// How many iterations the loop has completed.
    pub iteration: usize,
}

impl SessionContext {
    pub fn new(config: SessionConfig, workspace: WorkspaceInfo) -> Self {
        Self {
            config,
            workspace,
            history: vec![],
            memories: vec![],
            iteration: 0,
        }
    }

    pub fn push_message(&mut self, msg: Message) {
        self.history.push(msg);
    }

    pub fn has_reached_limit(&self) -> bool {
        self.iteration >= self.config.max_iterations
    }

    /// Build the system prompt from config + workspace info + loaded memories.
    pub fn system_prompt(&self) -> String {
        let mut parts = vec![];

        if let Some(sys) = &self.config.system_prompt {
            parts.push(sys.clone());
        }

        parts.push(self.workspace.summary());

        if !self.memories.is_empty() {
            parts.push("## Persistent Memory".to_string());
            for (name, content) in &self.memories {
                parts.push(format!("### {name}\n{content}"));
            }
        }

        parts.join("\n\n")
    }
}
