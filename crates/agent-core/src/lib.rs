pub mod agent;
pub mod builder;
pub mod loop_runner;
pub mod dispatch;

pub use agent::Agent;
pub use builder::AgentBuilder;

// Convenience re-exports for consumers
pub use agent_protocol::{AgentError, AgentEvent, SessionConfig, SessionId};
pub use agent_tools::{ApprovalCallback, ApprovalDecision, AutoApprove, AutoDeny, ToolRegistry};
pub use agent_llm::LlmProvider;
