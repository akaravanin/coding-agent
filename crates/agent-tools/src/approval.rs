use async_trait::async_trait;
use agent_protocol::ToolCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

/// Injected by the consumer to gate tool execution.
/// CLI: prompt the user. Automated runner: auto-approve or policy-check.
#[async_trait]
pub trait ApprovalCallback: Send + Sync {
    async fn request_approval(&self, call: &ToolCall) -> ApprovalDecision;
}

/// Convenience: always approve (use in tests or trusted environments).
pub struct AutoApprove;

#[async_trait]
impl ApprovalCallback for AutoApprove {
    async fn request_approval(&self, _call: &ToolCall) -> ApprovalDecision {
        ApprovalDecision::Approved
    }
}

/// Convenience: always deny (use in read-only or sandboxed sessions).
pub struct AutoDeny;

#[async_trait]
impl ApprovalCallback for AutoDeny {
    async fn request_approval(&self, _call: &ToolCall) -> ApprovalDecision {
        ApprovalDecision::Denied
    }
}
