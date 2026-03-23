use async_trait::async_trait;
use serde_json::json;
use tokio::process::Command;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::registry::Tool;
use crate::sandbox::Sandbox;

/// Executes shell commands within the workspace root.
/// Always requires approval. Disabled if `sandbox.allow_shell` is false.
pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "shell".into(),
            description: "Execute a shell command in the workspace root. Use for build, test, and scripting tasks.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Optional timeout in seconds (default: 30).",
                        "default": 30
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn requires_approval(&self) -> bool { true }

    async fn execute(&self, call: &ToolCall, sandbox: &Sandbox) -> Result<ToolResult, AgentError> {
        sandbox.check_shell()?;

        let command = call.input["command"]
            .as_str()
            .ok_or_else(|| AgentError::Tool {
                tool: "shell".into(),
                message: "Missing 'command' field".into(),
            })?;

        let timeout_secs = call.input["timeout_secs"].as_u64().unwrap_or(30);

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&sandbox.workspace_root)
                .output(),
        )
        .await
        .map_err(|_| AgentError::Tool {
            tool: "shell".into(),
            message: format!("Command timed out after {timeout_secs}s"),
        })?
        .map_err(|e| AgentError::Tool {
            tool: "shell".into(),
            message: e.to_string(),
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = json!({
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        });

        if output.status.success() {
            Ok(ToolResult::success(call.id.clone(), result))
        } else {
            // Return as error so the agent knows the command failed, but include output.
            Ok(ToolResult {
                call_id: call.id.clone(),
                tool_name: None,
                status: agent_protocol::ToolResultStatus::Error,
                output: result,
                display: Some(format!("exit {exit_code}: {stderr}")),
            })
        }
    }
}
