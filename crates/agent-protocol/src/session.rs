use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(uuid_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Per-session configuration injected by the consumer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: SessionId,
    /// Root directory the agent treats as its workspace.
    pub workspace_root: PathBuf,
    /// System prompt prepended before all user messages.
    pub system_prompt: Option<String>,
    /// Maximum number of agentic loop iterations before stopping.
    pub max_iterations: usize,
    /// Whether tools that require approval should block waiting for callback.
    pub require_approval: bool,
}

impl SessionConfig {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            id: SessionId::new(),
            workspace_root: workspace_root.into(),
            system_prompt: None,
            max_iterations: 50,
            require_approval: true,
        }
    }
}

/// Minimal UUID v4 without pulling in the uuid crate at protocol level.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("sess-{:x}-{:x}", t, pseudo_random())
}

fn pseudo_random() -> u64 {
    // Good enough for session IDs; swap for `rand` if needed.
    // Use a static atomic counter as a stable-Rust entropy source.
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0xdeadbeef_cafebabe);
    let v = CTR.fetch_add(0x9e3779b97f4a7c15, Ordering::Relaxed);
    v ^ (v >> 30)
}
