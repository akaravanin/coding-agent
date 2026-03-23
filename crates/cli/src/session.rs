use std::path::PathBuf;
use colored::Colorize;
use tokio::sync::broadcast::error::RecvError;

use agent_core::{AgentBuilder, AutoApprove, SessionConfig};
use agent_llm::{AnthropicProvider, GeminiProvider};
use agent_tools::sandbox::Sandbox;
use agent_protocol::AgentError;

use crate::approval::TerminalApproval;
use crate::display;

pub enum ProviderConfig {
    Anthropic { api_key: String, model: String },
    Gemini    { api_key: String, model: String },
}

pub struct Config {
    pub workspace: PathBuf,
    pub provider: ProviderConfig,
    pub auto_approve: bool,
}

/// Run a single prompt, print events, exit.
pub async fn run_once(config: Config, prompt: String) -> Result<(), AgentError> {
    let (provider_name, model) = config.provider.label();
    print_header(&config.workspace, provider_name, model);

    let agent = build_agent(config)?;
    let mut events = agent.subscribe();

    let run = tokio::spawn(async move {
        agent.run(prompt).await
    });

    loop {
        match events.recv().await {
            Ok(event) => { if !display::render(&event) { break; } }
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

    let (provider_name, model) = config.provider.label();
    print_header(&config.workspace, provider_name, model);
    println!("{}", "  Type your message and press Enter. Ctrl+C or /quit to exit.".dimmed());
    println!();

    // Extract auto_approve before config is consumed per-iteration
    let auto_approve = config.auto_approve;
    let workspace = config.workspace.clone();

    let stdin = io::stdin();
    loop {
        print!("{} ", "you ›".green().bold());
        io::stdout().flush().ok();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let prompt = line.trim().to_string();
        if prompt.is_empty() { continue; }
        if prompt == "/quit" || prompt == "/exit" { break; }

        println!();
        print!("{} ", "agent ›".blue().bold());

        // Rebuild config each turn (agent is consumed per session)
        let turn_config = Config {
            workspace: workspace.clone(),
            provider: rebuild_provider(&config.provider),
            auto_approve,
        };

        let agent = build_agent(turn_config)?;
        let mut events = agent.subscribe();

        let run = tokio::spawn(async move { agent.run(prompt).await });

        loop {
            match events.recv().await {
                Ok(event) => { if !display::render(&event) { break; } }
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

fn build_agent(config: Config) -> Result<agent_core::Agent, AgentError> {
    let sandbox = Sandbox::permissive(&config.workspace);
    let session = SessionConfig::new(&config.workspace);

    let builder = match config.provider {
        ProviderConfig::Anthropic { api_key, model } => {
            AgentBuilder::new()
                .provider(AnthropicProvider::new(api_key).with_model(model))
        }
        ProviderConfig::Gemini { api_key, model } => {
            AgentBuilder::new()
                .provider(GeminiProvider::new(api_key).with_model(model))
        }
    };

    let builder = builder.sandbox(sandbox).config(session);

    if config.auto_approve {
        builder.approval(AutoApprove).build()
    } else {
        builder.approval(TerminalApproval).build()
    }
}

impl ProviderConfig {
    fn label(&self) -> (&str, &str) {
        match self {
            ProviderConfig::Anthropic { model, .. } => ("anthropic", model.as_str()),
            ProviderConfig::Gemini    { model, .. } => ("gemini",    model.as_str()),
        }
    }
}

/// Clone provider config for REPL iterations without storing keys twice.
fn rebuild_provider(p: &ProviderConfig) -> ProviderConfig {
    match p {
        ProviderConfig::Anthropic { api_key, model } =>
            ProviderConfig::Anthropic { api_key: api_key.clone(), model: model.clone() },
        ProviderConfig::Gemini { api_key, model } =>
            ProviderConfig::Gemini { api_key: api_key.clone(), model: model.clone() },
    }
}

fn print_header(workspace: &PathBuf, provider: &str, model: &str) {
    println!();
    println!("{}  {}", "code-agent".blue().bold(), env!("CARGO_PKG_VERSION").dimmed());
    println!("{}  {}", "workspace:".dimmed(), workspace.display().to_string().cyan());
    println!("{}  {} / {}", "provider: ".dimmed(), provider.cyan(), model.cyan());
    println!();
}
