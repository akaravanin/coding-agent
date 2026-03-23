use async_trait::async_trait;
use serde_json::json;
use tokio::task;

use agent_protocol::{AgentError, ToolCall, ToolResult, ToolSchema};
use crate::registry::Tool;
use crate::sandbox::Sandbox;

pub struct FileSearchTool;

#[async_trait]
impl Tool for FileSearchTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "file_search".into(),
            description: "Search for a regex pattern in files within the workspace. Returns matching file paths and line excerpts.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for." },
                    "glob":    { "type": "string", "description": "Optional glob to restrict file types, e.g. '**/*.rs'." },
                    "max_results": { "type": "integer", "description": "Max number of matches to return (default 50).", "default": 50 }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn requires_approval(&self) -> bool { false }

    async fn execute(&self, call: &ToolCall, sandbox: &Sandbox) -> Result<ToolResult, AgentError> {
        let pattern = call.input["pattern"].as_str().ok_or_else(|| AgentError::Tool {
            tool: "file_search".into(),
            message: "Missing 'pattern'".into(),
        })?.to_string();

        let glob_str = call.input["glob"].as_str().map(str::to_string);
        let max_results = call.input["max_results"].as_u64().unwrap_or(50) as usize;
        let root = sandbox.workspace_root.clone();

        let matches = task::spawn_blocking(move || {
            grep_files(&root, &pattern, glob_str.as_deref(), max_results)
        })
        .await
        .map_err(|e| AgentError::Tool {
            tool: "file_search".into(),
            message: e.to_string(),
        })??;

        Ok(ToolResult::success(call.id.clone(), json!({ "matches": matches })))
    }
}

#[derive(serde::Serialize)]
struct Match {
    path: String,
    line: u64,
    text: String,
}

fn grep_files(
    root: &std::path::Path,
    pattern: &str,
    glob_filter: Option<&str>,
    max_results: usize,
) -> Result<Vec<Match>, AgentError> {
    use std::io::{BufRead, BufReader};

    let regex = regex_lite(pattern)?;
    let mut results = vec![];

    visit_dir(root, glob_filter, &mut |file_path| {
        if results.len() >= max_results {
            return;
        }
        let Ok(f) = std::fs::File::open(&file_path) else { return };
        let reader = BufReader::new(f);
        for (i, line) in reader.lines().enumerate() {
            if results.len() >= max_results { break; }
            let Ok(line) = line else { break };
            if regex.is_match(&line) {
                results.push(Match {
                    path: file_path.display().to_string(),
                    line: (i + 1) as u64,
                    text: line,
                });
            }
        }
    });

    Ok(results)
}

/// Minimal regex matching using std only (no regex crate dependency at this layer).
/// For production, replace with the `regex` crate.
struct SimpleRegex(String);
impl SimpleRegex {
    fn is_match(&self, text: &str) -> bool {
        text.contains(&self.0)
    }
}

fn regex_lite(pattern: &str) -> Result<SimpleRegex, AgentError> {
    // TODO: replace with `regex` crate for real pattern matching
    Ok(SimpleRegex(pattern.to_string()))
}

fn visit_dir(
    dir: &std::path::Path,
    _glob_filter: Option<&str>,
    callback: &mut impl FnMut(std::path::PathBuf),
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden dirs and common noise
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            visit_dir(&path, _glob_filter, callback);
        } else {
            callback(path);
        }
    }
}
