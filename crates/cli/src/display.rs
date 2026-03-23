use colored::Colorize;
use agent_protocol::{AgentEvent, ToolResultStatus};

/// Print an AgentEvent to stdout in a human-readable way.
/// Returns false if the session is complete or errored (caller should stop).
pub fn render(event: &AgentEvent) -> bool {
    match event {
        AgentEvent::ThinkingStarted => {
            print!("{}", "  thinking…".dimmed());
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        AgentEvent::ThinkingDone => {
            // Clear "thinking…" with a carriage return
            print!("\r{}\r", " ".repeat(14));
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        AgentEvent::MessageDelta { text } => {
            print!("{text}");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        AgentEvent::MessageComplete { .. } => {
            // Newline after streaming text finishes
            println!();
        }

        AgentEvent::ToolCallRequested { call, requires_approval } => {
            if !requires_approval {
                // Auto-approved tools get a lighter line
                println!(
                    "\n{}  {}  {}",
                    "⚙".cyan(),
                    call.name.cyan().bold(),
                    summarize_input(&call.input).dimmed()
                );
            }
            // Approval-required tools are handled by the approval prompt in approval.rs
        }

        AgentEvent::ToolCallApproved { .. } => {}  // already printed by approval prompt

        AgentEvent::ToolCallDenied { call_id } => {
            println!("{} {}", "  ✗ denied:".red(), call_id.dimmed());
        }

        AgentEvent::ToolCallCompleted { result } => {
            match result.status {
                ToolResultStatus::Success => {
                    if let Some(display) = &result.display {
                        println!("    {}", display.dimmed());
                    }
                }
                ToolResultStatus::Error => {
                    let msg = result.display.as_deref()
                        .unwrap_or_else(|| result.output.as_str().unwrap_or("error"));
                    println!("  {} {}", "✗".red(), msg.red());
                }
                ToolResultStatus::Denied => {}  // already shown
            }
        }

        AgentEvent::Warning { message } => {
            println!("{} {}", "  ⚠".yellow(), message.yellow());
        }

        AgentEvent::SessionError { error } => {
            eprintln!("\n{} {}", "error:".red().bold(), error);
            return false;
        }

        AgentEvent::SessionComplete => {
            return false;
        }
    }

    true
}

/// One-line summary of tool input for display.
fn summarize_input(input: &serde_json::Value) -> String {
    if let Some(obj) = input.as_object() {
        let parts: Vec<String> = obj.iter()
            .take(2)
            .map(|(k, v)| {
                let val = match v {
                    serde_json::Value::String(s) => truncate(s, 40),
                    other => truncate(&other.to_string(), 40),
                };
                format!("{k}={val}")
            })
            .collect();
        parts.join(" ")
    } else {
        truncate(&input.to_string(), 60)
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
