use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tokio::fs;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::registry::Tool;
use crate::sandbox::Sandbox;

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "file_write".into(),
            description: "Write or overwrite a file. Creates parent directories as needed. Path is relative to workspace root.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path":    { "type": "string", "description": "Destination path." },
                    "content": { "type": "string", "description": "Full file content to write." }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn requires_approval(&self) -> bool { true }

    async fn execute(&self, call: &ToolCall, sandbox: &Sandbox) -> Result<ToolResult, AgentError> {
        let raw_path = call.input["path"].as_str().ok_or_else(|| AgentError::Tool {
            tool: "file_write".into(),
            message: "Missing 'path'".into(),
        })?;
        let content = call.input["content"].as_str().ok_or_else(|| AgentError::Tool {
            tool: "file_write".into(),
            message: "Missing 'content'".into(),
        })?;

        let path = sandbox.check_write(Path::new(raw_path))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(AgentError::Io)?;
        }

        fs::write(&path, content).await.map_err(AgentError::Io)?;

        Ok(ToolResult::success(call.id.clone(), json!({ "written": path.display().to_string() })))
    }
}
