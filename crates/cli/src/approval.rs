use async_trait::async_trait;
use colored::Colorize;
use agent_protocol::ToolCall;
use agent_tools::{ApprovalCallback, ApprovalDecision};

/// Prompts the terminal user before each sensitive tool call.
pub struct TerminalApproval;

#[async_trait]
impl ApprovalCallback for TerminalApproval {
    async fn request_approval(&self, call: &ToolCall) -> ApprovalDecision {
        // Format the tool call for display
        let input_str = serde_json::to_string_pretty(&call.input)
            .unwrap_or_else(|_| call.input.to_string());

        println!();
        println!("{}", "┌─ Tool approval required ───────────────────".yellow());
        println!("{}  {}", "│ Tool:".yellow(), call.name.bold());
        for line in input_str.lines() {
            println!("{}  {}", "│".yellow(), line.dimmed());
        }
        println!("{}", "└────────────────────────────────────────────".yellow());
        print!("{}", "  Allow? [y/N] ".yellow().bold());

        use std::io::{self, Write};
        io::stdout().flush().ok();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                if input.trim().eq_ignore_ascii_case("y") {
                    println!("{}", "  ✓ Approved".green());
                    ApprovalDecision::Approved
                } else {
                    println!("{}", "  ✗ Denied".red());
                    ApprovalDecision::Denied
                }
            }
            Err(_) => {
                // Non-interactive stdin (piped) — deny by default
                println!("{}", "  ✗ Denied (non-interactive)".red());
                ApprovalDecision::Denied
            }
        }
    }
}
