mod approval;
mod display;
mod session;

use std::path::PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
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

    /// LLM provider to use.
    #[arg(long, global = true, default_value = "anthropic", env = "CODE_AGENT_PROVIDER")]
    provider: Provider,

    /// Model to use (defaults per provider: claude-sonnet-4-6 / gemini-2.0-flash).
    #[arg(long, global = true, env = "CODE_AGENT_MODEL")]
    model: Option<String>,

    /// Anthropic API key.
    #[arg(long, global = true, env = "ANTHROPIC_API_KEY", hide_env_values = true)]
    anthropic_key: Option<String>,

    /// Gemini API key.
    #[arg(long, global = true, env = "GEMINI_API_KEY", hide_env_values = true)]
    gemini_key: Option<String>,

    /// Auto-approve all tool calls without prompting.
    #[arg(long, global = true)]
    yes: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Provider {
    Anthropic,
    Gemini,
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

    let config = match cli.provider {
        Provider::Anthropic => {
            let api_key = match cli.anthropic_key {
                Some(k) => k,
                None => {
                    eprintln!("error: ANTHROPIC_API_KEY not set.");
                    std::process::exit(1);
                }
            };
            session::Config {
                workspace,
                provider: session::ProviderConfig::Anthropic {
                    api_key,
                    model: cli.model.unwrap_or_else(|| "claude-sonnet-4-6".into()),
                },
                auto_approve: cli.yes,
            }
        }
        Provider::Gemini => {
            let api_key = match cli.gemini_key {
                Some(k) => k,
                None => {
                    eprintln!("error: GEMINI_API_KEY not set.");
                    std::process::exit(1);
                }
            };
            session::Config {
                workspace,
                provider: session::ProviderConfig::Gemini {
                    api_key,
                    model: cli.model.unwrap_or_else(|| "gemini-2.0-flash".into()),
                },
                auto_approve: cli.yes,
            }
        }
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
