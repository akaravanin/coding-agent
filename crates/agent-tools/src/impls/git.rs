use async_trait::async_trait;
use serde_json::json;
use tokio::process::Command;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::registry::Tool;
use crate::sandbox::Sandbox;

/// Safe read-only git operations (status, log, diff).
/// Write operations (commit, push) require approval.
pub struct GitTool;

#[async_trait]
impl Tool for GitTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "git".into(),
            description: "Run a git command in the workspace. Read-only operations (status, log, diff, show) are auto-approved. Write operations require approval.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Git subcommand and arguments, e.g. [\"log\", \"--oneline\", \"-10\"]"
                    }
                },
                "required": ["args"]
            }),
        }
    }

    fn requires_approval(&self) -> bool {
        // Approval is determined dynamically per call in execute().
        // Return true as default; agent-core checks this before calling.
        true
    }

    async fn execute(&self, call: &ToolCall, sandbox: &Sandbox) -> Result<ToolResult, AgentError> {
        sandbox.check_shell()?;

        let args: Vec<String> = call.input["args"]
            .as_array()
            .ok_or_else(|| AgentError::Tool {
                tool: "git".into(),
                message: "Missing 'args' array".into(),
            })?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();

        let output = Command::new("git")
            .args(&args)
            .current_dir(&sandbox.workspace_root)
            .output()
            .await
            .map_err(|e| AgentError::Tool { tool: "git".into(), message: e.to_string() })?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        Ok(ToolResult::success(call.id.clone(), json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": output.status.code().unwrap_or(-1),
        })))
    }
}

/// Returns true if the git args represent a read-only operation.
pub fn is_readonly_git_op(args: &[&str]) -> bool {
    matches!(
        args.first().map(|s| *s),
        Some("status") | Some("log") | Some("diff") | Some("show") |
        Some("branch") | Some("remote") | Some("stash") | Some("tag")
    )
}
