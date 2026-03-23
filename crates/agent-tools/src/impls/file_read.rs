use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tokio::fs;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::registry::Tool;
use crate::sandbox::Sandbox;

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "file_read".into(),
            description: "Read the contents of a file. Path is relative to workspace root.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read." },
                    "start_line": { "type": "integer", "description": "1-based line to start from (optional)." },
                    "end_line":   { "type": "integer", "description": "1-based line to end at (optional, inclusive)." }
                },
                "required": ["path"]
            }),
        }
    }

    fn requires_approval(&self) -> bool { false }

    async fn execute(&self, call: &ToolCall, sandbox: &Sandbox) -> Result<ToolResult, AgentError> {
        let raw_path = call.input["path"].as_str().ok_or_else(|| AgentError::Tool {
            tool: "file_read".into(),
            message: "Missing 'path'".into(),
        })?;

        let path = sandbox.check_read(Path::new(raw_path))?;
        let content = fs::read_to_string(&path).await.map_err(AgentError::Io)?;

        let content = apply_line_range(
            &content,
            call.input["start_line"].as_u64(),
            call.input["end_line"].as_u64(),
        );

        Ok(ToolResult::success(call.id.clone(), json!({ "content": content })))
    }
}

fn apply_line_range(content: &str, start: Option<u64>, end: Option<u64>) -> String {
    if start.is_none() && end.is_none() {
        return content.to_string();
    }
    let lines: Vec<&str> = content.lines().collect();
    let s = start.map(|n| (n as usize).saturating_sub(1)).unwrap_or(0);
    let e = end.map(|n| n as usize).unwrap_or(lines.len());
    lines[s.min(lines.len())..e.min(lines.len())].join("\n")
}
