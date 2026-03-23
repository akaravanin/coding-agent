mod approval;
mod display;
mod session;

use std::path::PathBuf;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// code-agent — headless code agent, CLI frontend.
#[derive(Parser)]
#[command(name = "code-agent", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Workspace directory (defaults to current directory).
    #[arg(short, long, global = true, env = "CODE_AGENT_WORKSPACE")]
    workspace: Option<PathBuf>,

    /// Anthropic API key (or set ANTHROPIC_API_KEY).
    #[arg(long, global = true, env = "ANTHROPIC_API_KEY", hide_env_values = true)]
    anthropic_key: Option<String>,

    /// Claude model to use.
    #[arg(long, global = true, default_value = "claude-sonnet-4-6", env = "CODE_AGENT_MODEL")]
    model: String,

    /// Auto-approve all tool calls without prompting.
    #[arg(long, global = true)]
    yes: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single prompt and exit.
    Run {
        /// The task to perform.
        prompt: String,
    },
    /// Start an interactive chat session (REPL).
    Chat,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .without_time()
        .init();

    let workspace = cli.workspace
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));

    let api_key = match cli.anthropic_key {
        Some(k) => k,
        None => {
            eprintln!("error: ANTHROPIC_API_KEY not set. Pass --anthropic-key or set the env var.");
            std::process::exit(1);
        }
    };

    let config = session::Config {
        workspace,
        api_key,
        model: cli.model,
        auto_approve: cli.yes,
    };

    let result = match cli.command.unwrap_or(Command::Chat) {
        Command::Run { prompt } => session::run_once(config, prompt).await,
        Command::Chat => session::run_chat(config).await,
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
