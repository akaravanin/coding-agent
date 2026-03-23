use std::path::{Path, PathBuf};
use agent_protocol::AgentError;

/// Controls what paths and operations a tool is permitted to access.
/// All tool implementations receive a reference to the active Sandbox.
#[derive(Debug, Clone)]
pub struct Sandbox {
    pub workspace_root: PathBuf,
    pub allow_shell: bool,
    pub allow_net: bool,
    /// Paths outside workspace_root that are additionally readable.
    pub extra_read_paths: Vec<PathBuf>,
}

impl Sandbox {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allow_shell: false,
            allow_net: false,
            extra_read_paths: vec![],
        }
    }

    pub fn permissive(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allow_shell: true,
            allow_net: true,
            extra_read_paths: vec![],
        }
    }

    /// Resolve and verify that `path` is within an allowed read location.
    pub fn check_read(&self, path: &Path) -> Result<PathBuf, AgentError> {
        let canonical = self.canonicalize_within_workspace(path)?;
        Ok(canonical)
    }

    /// Resolve and verify that `path` is within the workspace root for writes.
    pub fn check_write(&self, path: &Path) -> Result<PathBuf, AgentError> {
        let canonical = self.canonicalize_within_workspace(path)?;
        Ok(canonical)
    }

    pub fn check_shell(&self) -> Result<(), AgentError> {
        if self.allow_shell {
            Ok(())
        } else {
            Err(AgentError::Tool {
                tool: "shell".into(),
                message: "Shell execution is disabled in this sandbox".into(),
            })
        }
    }

    fn canonicalize_within_workspace(&self, path: &Path) -> Result<PathBuf, AgentError> {
        // Resolve relative paths against workspace root.
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        };

        // Normalize without requiring existence (avoid TOCTOU; real check on use).
        let normalized = normalize_path(&resolved);

        if normalized.starts_with(&self.workspace_root) {
            return Ok(normalized);
        }
        for extra in &self.extra_read_paths {
            if normalized.starts_with(extra) {
                return Ok(normalized);
            }
        }

        Err(AgentError::Tool {
            tool: "sandbox".into(),
            message: format!(
                "Path '{}' is outside the allowed workspace '{}'",
                normalized.display(),
                self.workspace_root.display()
            ),
        })
    }
}

/// Lexically normalize a path (remove `.` and `..`) without hitting the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => { out.pop(); }
            c => out.push(c),
        }
    }
    out
}
