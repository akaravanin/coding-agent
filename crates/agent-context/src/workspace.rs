use std::path::{Path, PathBuf};
use tokio::fs;
use agent_protocol::AgentError;

/// Static facts about the workspace, computed once at session start.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub root: PathBuf,
    pub is_git_repo: bool,
    pub git_branch: Option<String>,
    pub language_hints: Vec<String>,
}

impl WorkspaceInfo {
    pub async fn detect(root: impl Into<PathBuf>) -> Result<Self, AgentError> {
        let root = root.into();

        let is_git_repo = root.join(".git").exists();
        let git_branch = if is_git_repo {
            read_git_branch(&root).await
        } else {
            None
        };

        let language_hints = detect_languages(&root).await;

        Ok(Self { root, is_git_repo, git_branch, language_hints })
    }

    /// Human-readable summary injected into the system prompt.
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("Workspace: {}", self.root.display()),
        ];
        if self.is_git_repo {
            let branch = self.git_branch.as_deref().unwrap_or("unknown");
            lines.push(format!("Git repo, branch: {branch}"));
        }
        if !self.language_hints.is_empty() {
            lines.push(format!("Detected languages: {}", self.language_hints.join(", ")));
        }
        lines.join("\n")
    }
}

async fn read_git_branch(root: &Path) -> Option<String> {
    let head = root.join(".git").join("HEAD");
    let content = fs::read_to_string(head).await.ok()?;
    let content = content.trim();
    if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
        Some(branch.to_string())
    } else {
        Some(content[..8.min(content.len())].to_string()) // detached HEAD
    }
}

async fn detect_languages(root: &Path) -> Vec<String> {
    let indicators = [
        ("Cargo.toml", "Rust"),
        ("package.json", "JavaScript/TypeScript"),
        ("go.mod", "Go"),
        ("pyproject.toml", "Python"),
        ("pom.xml", "Java"),
        ("build.gradle", "Java/Kotlin"),
    ];

    let mut langs = vec![];
    for (file, lang) in &indicators {
        if root.join(file).exists() {
            langs.push(lang.to_string());
        }
    }
    langs
}
