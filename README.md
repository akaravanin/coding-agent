# code-agent

A headless, generic code agent engine in Rust. No CLI, no UI baked in — consumers connect separately via async channels and an event stream.

## What it is

`code-agent` is a library that runs an agentic loop: it takes a user message, calls an LLM, dispatches tool calls (with approval gating), feeds results back, and repeats until the task is done. You wire up your own frontend — a CLI binary, a web server, an IDE extension — by subscribing to the event stream and injecting an approval callback.

```
Your CLI / UI / Server
        │
        │  send message
        ▼
   ┌─────────────┐      LlmProvider trait     ┌──────────────┐
   │  agent-core │ ◄──────────────────────── │  Anthropic   │
   │  (loop)     │                            │  Gemini      │
   └──────┬──────┘                            └──────────────┘
          │ ApprovalCallback
          │ (you implement this)
          ▼
   ┌─────────────┐
   │ agent-tools │  file_read · file_write · shell · git · memory
   └─────────────┘
          │
          ▼
   broadcast::Receiver<AgentEvent>
   (text deltas, tool calls, results, errors)
```

## Repository layout

```
coding-agent/
├── crates/
│   ├── agent-protocol/   # Shared types only. No I/O. Every other crate depends on this.
│   ├── agent-llm/        # LlmProvider trait + Anthropic impl. Gemini stub ready.
│   ├── agent-tools/      # Tool trait, registry, sandbox, approval callback, 6 built-in tools.
│   ├── agent-context/    # WorkspaceInfo, SessionContext, FileIndex.
│   ├── agent-core/       # AgentBuilder, agentic loop, tool dispatch, event channel.
│   └── cli/              # code-agent binary — terminal frontend for the engine.
├── scripts/
│   └── start.sh          # Build image + launch CLI in one command.
├── Dockerfile
└── docker-compose.yml
```

## Getting started

The fastest way — no Rust installation required:

```bash
git clone <repo>
cd coding-agent

export ANTHROPIC_API_KEY=sk-...
export GEMINI_API_KEY=AIza...

./scripts/start.sh                               # Anthropic, interactive chat
./scripts/start.sh --provider gemini             # Gemini, interactive chat
./scripts/start.sh "fix the failing test"        # one-shot run
./scripts/start.sh --provider gemini "explain this codebase"
./scripts/start.sh --workspace /my/project       # different workspace
./scripts/start.sh --yes "add docs"              # auto-approve all tool calls
./scripts/start.sh --rebuild                     # force Docker image rebuild
```

`start.sh` builds the Docker image on first run, mounts your workspace into the container, and passes through API keys automatically.

## CLI reference

```
Usage: code-agent [OPTIONS] [COMMAND]

Commands:
  run <PROMPT>   Run a single prompt and exit
  chat           Interactive REPL session (default when no command given)

Options:
  -w, --workspace <DIR>      Workspace directory [default: current dir]
      --provider <PROVIDER>  LLM provider: anthropic, gemini [default: anthropic]
      --model <MODEL>        Model override (defaults per provider)
      --yes                  Auto-approve all tool calls without prompting
      --anthropic-key        Anthropic API key (or ANTHROPIC_API_KEY)
      --gemini-key           Gemini API key (or GEMINI_API_KEY)
```

**One-shot:**
```bash
code-agent run "refactor the auth module to use async/await"
```

**Interactive:**
```bash
code-agent chat
# or just:
code-agent
```

**Via Docker compose directly:**
```bash
docker compose build cli
ANTHROPIC_API_KEY=sk-... WORKSPACE=/path/to/project docker compose run --rm cli chat
```

## Crates

| Crate | Role |
|-------|------|
| `agent-protocol` | `Message`, `ToolCall`, `ToolResult`, `AgentEvent`, `SessionConfig`. Pure data, no I/O. |
| `agent-llm` | `LlmProvider` trait, `LlmRequest`/`LlmResponse`, streaming events. `AnthropicProvider` + `GeminiProvider` stub. |
| `agent-tools` | `Tool` trait, `ToolRegistry`, `Sandbox` (path enforcement), `ApprovalCallback`. Six built-in tools. |
| `agent-context` | `WorkspaceInfo` (auto-detects language/git branch), `SessionContext` (in-memory history + memories), `FileIndex`. |
| `agent-core` | `AgentBuilder`, `Agent` handle, `LoopRunner` (agentic loop), `dispatch` (approval → execute). |
| `cli` | `code-agent` binary. `clap` arg parsing, terminal approval prompts, coloured event display. |

## Using the library

```rust
use agent_core::{AgentBuilder, AutoApprove, SessionConfig};
use agent_llm::AnthropicProvider;

#[tokio::main]
async fn main() {
    let agent = AgentBuilder::new()
        .provider(AnthropicProvider::new(std::env::var("ANTHROPIC_API_KEY").unwrap()))
        .approval(AutoApprove)           // or your own ApprovalCallback impl
        .config(SessionConfig::new("/path/to/workspace"))
        .build()
        .unwrap();

    // Subscribe before run() to avoid missing early events
    let mut events = agent.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("{event:?}");
        }
    });

    agent.run("Fix the failing tests in src/lib.rs").await.unwrap();
}
```

## How tool calling works

When you send a message, the agent runs a loop until the task is complete:

```
you: "add a README to this project"
         │
         ▼
  ┌─────────────────────────────────────────────────────────┐
  │  1. Build request                                        │
  │     history + system prompt + all tool schemas → LLM    │
  └──────────────────────┬──────────────────────────────────┘
                         │
                         ▼
  ┌─────────────────────────────────────────────────────────┐
  │  2. LLM responds with a mix of text and tool calls       │
  │                                                          │
  │   "I'll start by reading the existing files."            │
  │   [tool_use: file_search { pattern: "README" }]          │
  └──────────────────────┬──────────────────────────────────┘
                         │
                         ▼
  ┌─────────────────────────────────────────────────────────┐
  │  3. For each tool call:                                  │
  │                                                          │
  │   a) Does this tool require approval?                    │
  │      file_search → no  → execute immediately            │
  │      file_write  → yes → ask ApprovalCallback           │
  │                            approved → execute            │
  │                            denied   → return Denied      │
  │                                                          │
  │   b) Sandbox checks path is inside workspace root        │
  │   c) Tool executes, returns ToolResult                   │
  │   d) AgentEvent emitted for each stage                   │
  └──────────────────────┬──────────────────────────────────┘
                         │
                         ▼
  ┌─────────────────────────────────────────────────────────┐
  │  4. All results appended to history as a Tool message    │
  │     Loop back to step 1                                  │
  └──────────────────────┬──────────────────────────────────┘
                         │
                         ▼ (LLM returns EndTurn — no more tool calls)
  ┌─────────────────────────────────────────────────────────┐
  │  5. Final text response, SessionComplete event           │
  └─────────────────────────────────────────────────────────┘
```

**What the LLM actually sends** — Claude uses Anthropic's tool use format. A response containing a tool call looks like this (simplified):

```json
{
  "role": "assistant",
  "content": [
    { "type": "text", "text": "Let me check what files exist first." },
    {
      "type": "tool_use",
      "id": "call_01",
      "name": "file_search",
      "input": { "pattern": "README", "glob": "**/*.md" }
    }
  ],
  "stop_reason": "tool_use"
}
```

`stop_reason: "tool_use"` tells the loop there are tool calls to dispatch before the LLM can continue. When the LLM is done it returns `stop_reason: "end_turn"` and the loop exits.

**What goes back to the LLM** — after all tool calls in a turn are executed, their results are appended as a single `Tool`-role message:

```json
{
  "role": "tool",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "call_01",
      "content": "{ \"matches\": [{\"path\": \"docs/old-readme.md\", \"line\": 1}] }"
    }
  ]
}
```

The full conversation history (user → assistant → tool → assistant → …) is sent on every iteration, giving the LLM complete context of what it has done so far.

**Multiple tool calls per turn** — the LLM can request several tools in one response. They are dispatched sequentially (respecting approval for each), and all results are collected before the next LLM call.

**What can go wrong** — `ToolResult` has three statuses:
- `Success` — executed cleanly, output returned to LLM
- `Error` — tool ran but failed (bad path, command exited non-zero, etc.), error message returned to LLM so it can try a different approach
- `Denied` — approval callback said no; LLM is told the tool was denied and can ask the user what to do next

## Approval callback

Every tool that touches the filesystem or runs shell commands calls your `ApprovalCallback` before executing. The CLI implements a terminal prompt; you can implement any policy:

```rust
use agent_tools::{ApprovalCallback, ApprovalDecision};
use agent_protocol::ToolCall;
use async_trait::async_trait;

struct PromptUser;

#[async_trait]
impl ApprovalCallback for PromptUser {
    async fn request_approval(&self, call: &ToolCall) -> ApprovalDecision {
        println!("Allow '{}' with input {}? [y/N]", call.name, call.input);
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        if line.trim() == "y" { ApprovalDecision::Approved } else { ApprovalDecision::Denied }
    }
}
```

Built-in callbacks: `AutoApprove` (tests / trusted env), `AutoDeny` (read-only sessions).

## Built-in tools

| Tool | Approval | Description |
|------|:--------:|-------------|
| `file_read` | no | Read file contents, optionally by line range |
| `file_write` | **yes** | Write or overwrite a file, creates parent dirs |
| `file_search` | no | Search files by pattern |
| `shell` | **yes** | Run a bash command in the workspace |
| `git` | **yes** | Run git commands |
| `memory_write` | no | Persist a markdown note to `memory/` (gitignored) |

## Memory files

The agent can write persistent notes to `{workspace}/memory/*.md` via `memory_write`. They are:
- Loaded at session start and injected into the system prompt
- Free-form markdown, entirely agent-managed
- Excluded from git via `.gitignore`
- The only form of persistence across sessions (everything else is in-memory)

## LLM providers

| Provider | Status | Default model |
|----------|--------|---------------|
| Anthropic (Claude) | Implemented | `claude-sonnet-4-6` |
| Google Gemini | Implemented | `gemini-2.0-flash` |

Adding a provider means implementing one trait:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, AgentError>;
    async fn stream(&self, request: LlmRequest) -> Result<StreamResult, AgentError>;
}
```

## Event stream

```rust
pub enum AgentEvent {
    ThinkingStarted,
    ThinkingDone,
    MessageDelta { text: String },
    MessageComplete { message: Message },
    ToolCallRequested { call: ToolCall, requires_approval: bool },
    ToolCallApproved { call_id: String },
    ToolCallDenied { call_id: String },
    ToolCallCompleted { result: ToolResult },
    Warning { message: String },
    SessionError { error: String },
    SessionComplete,
}
```

## Development

No local Rust installation needed — everything runs in Docker.

```bash
docker compose run check    # cargo check
docker compose run test     # cargo test
docker compose run clippy   # cargo clippy -- -D warnings
```

Source is volume-mounted so edits are picked up immediately without rebuilding the image. Cargo registry and build cache are persisted in named volumes across runs.

## Design principles

- **Headless** — `agent-core` is a library. No `main()`, no stdin/stdout assumptions.
- **Protocol-first** — all shared types live in `agent-protocol`; other crates depend on it, it depends on nothing internal.
- **Generic LLM** — swap providers via the `LlmProvider` trait without touching core logic.
- **Sandboxed tools** — tools are path-restricted to the workspace; shell execution is opt-in; all writes go through approval.
- **In-memory sessions** — no database; state doesn't persist across restarts (only `memory/` files do).
- **No panics in library code** — `Result<T, AgentError>` everywhere; panics only in `main()`.
- **Async everywhere** — Tokio runtime; blocking ops run in `spawn_blocking`.

## License

MIT
