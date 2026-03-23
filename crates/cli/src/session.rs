use std::path::PathBuf;
use colored::Colorize;
use tokio::sync::broadcast::error::RecvError;

use agent_core::{AgentBuilder, AutoApprove, SessionConfig};
use agent_llm::AnthropicProvider;
use agent_tools::sandbox::Sandbox;
use agent_protocol::AgentError;

use crate::approval::TerminalApproval;
use crate::display;

pub struct Config {
    pub workspace: PathBuf,
    pub api_key: String,
    pub model: String,
    pub auto_approve: bool,
}

/// Run a single prompt, print events, exit.
pub async fn run_once(config: Config, prompt: String) -> Result<(), AgentError> {
    let agent = build_agent(&config)?;
    let mut events = agent.subscribe();

    print_header(&config.workspace);

    let run = tokio::spawn(async move {
        agent.run(prompt).await
    });

    loop {
        match events.recv().await {
            Ok(event) => {
                if !display::render(&event) { break; }
            }
            Err(RecvError::Closed) => break,
            Err(RecvError::Lagged(n)) => {
                eprintln!("{} dropped {n} events (channel lagged)", "warn:".yellow());
            }
        }
    }

    run.await.map_err(|e| AgentError::Session(e.to_string()))?
}

/// Interactive REPL: read a line, run the agent, repeat.
pub async fn run_chat(config: Config) -> Result<(), AgentError> {
    use std::io::{self, BufRead, Write};

    print_header(&config.workspace);
    println!("{}", "  Type your message and press Enter. Ctrl+C to quit.".dimmed());
    println!();

    let stdin = io::stdin();
    loop {
        print!("{} ", "you ›".green().bold());
        io::stdout().flush().ok();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break, // EOF / Ctrl+D
            Ok(_) => {}
        }

        let prompt = line.trim().to_string();
        if prompt.is_empty() { continue; }
        if prompt == "/quit" || prompt == "/exit" { break; }

        println!();
        print!("{} ", "agent ›".blue().bold());

        let agent = build_agent(&config)?;
        let mut events = agent.subscribe();

        let run = tokio::spawn(async move {
            agent.run(prompt).await
        });

        loop {
            match events.recv().await {
                Ok(event) => {
                    if !display::render(&event) { break; }
                }
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(n)) => {
                    eprintln!("{} dropped {n} events", "warn:".yellow());
                }
            }
        }

        run.await.map_err(|e| AgentError::Session(e.to_string()))??;
        println!();
    }

    Ok(())
}

fn build_agent(config: &Config) -> Result<agent_core::Agent, AgentError> {
    let provider = AnthropicProvider::new(&config.api_key)
        .with_model(&config.model);

    let sandbox = Sandbox::permissive(&config.workspace);

    let session = SessionConfig::new(&config.workspace);

    let builder = AgentBuilder::new()
        .provider(provider)
        .sandbox(sandbox)
        .config(session);

    let builder = if config.auto_approve {
        builder.approval(AutoApprove)
    } else {
        builder.approval(TerminalApproval)
    };

    builder.build()
}

fn print_header(workspace: &PathBuf) {
    println!();
    println!("{}  {}", "code-agent".blue().bold(), env!("CARGO_PKG_VERSION").dimmed());
    println!("{}  {}", "workspace:".dimmed(), workspace.display().to_string().cyan());
    println!();
}
