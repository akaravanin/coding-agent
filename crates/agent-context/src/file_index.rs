use std::path::{Path, PathBuf};
use tokio::task;
use agent_protocol::AgentError;

/// Lightweight index of files in the workspace.
/// Used to give the agent an overview without reading all content.
#[derive(Debug, Default)]
pub struct FileIndex {
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
}

impl FileIndex {
    pub async fn build(root: impl Into<PathBuf>) -> Result<Self, AgentError> {
        let root = root.into();
        let entries = task::spawn_blocking(move || collect_files(&root))
            .await
            .map_err(|e| AgentError::Context(e.to_string()))??;
        Ok(Self { entries })
    }

    /// Compact tree-style summary for the system prompt.
    pub fn tree_summary(&self, max_files: usize) -> String {
        let mut lines: Vec<String> = self.entries
            .iter()
            .take(max_files)
            .map(|e| format!("  {}", e.path.display()))
            .collect();

        if self.entries.len() > max_files {
            lines.push(format!("  ... and {} more files", self.entries.len() - max_files));
        }
        lines.join("\n")
    }
}

fn collect_files(root: &Path) -> Result<Vec<FileEntry>, AgentError> {
    let mut entries = vec![];
    visit(root, root, &mut entries);
    Ok(entries)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<FileEntry>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            visit(root, &path, out);
        } else {
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(FileEntry { path: rel, size_bytes });
        }
    }
}
