use async_trait::async_trait;
use serde_json::json;
use tokio::fs;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::registry::Tool;
use crate::sandbox::Sandbox;

/// Writes markdown memory files to `{workspace_root}/memory/`.
/// These are gitignored and persist across sessions.
pub struct MemoryTool;

#[async_trait]
impl Tool for MemoryTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "memory_write".into(),
            description: "Persist a note to a named memory file (markdown). Memories survive session restarts. Files are gitignored.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name":    { "type": "string", "description": "Memory file name (no extension, alphanumeric + hyphens)." },
                    "content": { "type": "string", "description": "Markdown content to write." },
                    "append":  { "type": "boolean", "description": "If true, append instead of overwrite (default: false).", "default": false }
                },
                "required": ["name", "content"]
            }),
        }
    }

    fn requires_approval(&self) -> bool { false }

    async fn execute(&self, call: &ToolCall, sandbox: &Sandbox) -> Result<ToolResult, AgentError> {
        let name = call.input["name"].as_str().ok_or_else(|| AgentError::Tool {
            tool: "memory_write".into(),
            message: "Missing 'name'".into(),
        })?;

        // Sanitize name: only allow safe characters
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(AgentError::Tool {
                tool: "memory_write".into(),
                message: format!("Invalid memory name '{name}': use only alphanumeric, hyphens, underscores"),
            });
        }

        let content = call.input["content"].as_str().ok_or_else(|| AgentError::Tool {
            tool: "memory_write".into(),
            message: "Missing 'content'".into(),
        })?;

        let append = call.input["append"].as_bool().unwrap_or(false);
        let memory_dir = sandbox.workspace_root.join("memory");
        fs::create_dir_all(&memory_dir).await.map_err(AgentError::Io)?;

        let file_path = memory_dir.join(format!("{name}.md"));

        if append {
            let existing = fs::read_to_string(&file_path).await.unwrap_or_default();
            fs::write(&file_path, format!("{existing}\n{content}")).await.map_err(AgentError::Io)?;
        } else {
            fs::write(&file_path, content).await.map_err(AgentError::Io)?;
        }

        Ok(ToolResult::success(call.id.clone(), json!({ "saved": file_path.display().to_string() })))
    }
}

/// Reads all memory files from `{workspace_root}/memory/`.
pub async fn load_memories(workspace_root: &std::path::Path) -> Vec<(String, String)> {
    let memory_dir = workspace_root.join("memory");
    let Ok(mut entries) = fs::read_dir(&memory_dir).await else { return vec![] };

    let mut memories = vec![];
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if let Ok(content) = fs::read_to_string(&path).await {
                memories.push((name, content));
            }
        }
    }
    memories
}
