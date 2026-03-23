# code-agent

A headless, generic code agent engine in Rust. No CLI, no UI — consumers connect separately.

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
├── Dockerfile            # Multi-stage: builder (rust:1.87-slim) → runtime (debian:bookworm-slim)
└── docker-compose.yml    # Services: check, test, clippy, cli
```

## Design Principles

1. **Headless engine** — `agent-core` is a library. CLI and UI are separate consumers that
   drive the agent via async channels and listen to an event stream.

2. **Generic LLM backend** — All LLM access goes through the `LlmProvider` trait in
   `agent-llm`. Anthropic is first; Gemini follows. Never hardcode provider logic in core.

3. **Sandboxed tools with approval** — Every tool that touches the filesystem, network,
   or shell must go through an `ApprovalCallback`. The callback is injected by the consumer
   (CLI prompts the user; automated runners can auto-approve or deny). Tool logic never
   self-approves.

4. **In-memory session state** — Agent state lives in memory for the duration of a session.
   No database. Sessions are not persisted across restarts.

5. **Memory files are opt-in and gitignored** — The agent may write persistent memory to
   `memory/` files in the workspace. These are excluded from git via `.gitignore`. This is
   the only form of cross-session persistence.

6. **Protocol-first** — All message types, tool schemas, and event shapes live in
   `agent-protocol`. Other crates depend on it; it depends on nothing internal.

7. **No panics in library code** — Use `Result<T, AgentError>` everywhere. Panics are only
   acceptable in `main()` entry points of consumer binaries.

8. **Async everywhere** — Tokio runtime. All I/O is async. Blocking operations (shell exec,
   file scan) run in `tokio::task::spawn_blocking`.

## Crate Dependency Order

```
agent-protocol
    ├── agent-llm
    ├── agent-tools
    ├── agent-context
    └── agent-core  (depends on all of the above)
         └── cli    (depends on agent-core)
```

## Key Types (agent-protocol)

- `Message` / `MessageRole` / `ContentBlock` — conversation turns; content can be text, tool use, or tool result
- `ToolCall` — LLM request to invoke a tool (`id`, `name`, `input: Value`)
- `ToolResult` — outcome of a tool call (`Success`, `Error`, `Denied`)
- `ToolSchema` — JSON Schema sent to the LLM describing a tool
- `AgentEvent` — broadcast events emitted by the loop (see Event Stream section)
- `SessionId` / `SessionConfig` — per-session identity and settings

## Agentic Loop (agent-core)

```
AgentBuilder::build() → Agent
    │
    agent.run(prompt)
    │
    └── LoopRunner::run(SessionContext)
            1. Build LlmRequest from history + system prompt + tool schemas
            2. provider.complete(request) → LlmResponse
            3. Emit ThinkingStarted/Done, MessageDelta, MessageComplete events
            4. Extract ToolCalls from response content blocks
            5. For each ToolCall → dispatch::dispatch_tool_call()
               a. Check tool.requires_approval()
               b. If yes → ApprovalCallback → Approved or Denied
               c. Execute tool with Sandbox constraints
               d. Emit ToolCallRequested/Approved/Denied/Completed events
            6. Append ToolResults to history as Tool-role message
            7. Loop back to step 1 until stop_reason == EndTurn or max_iterations hit
```

## Tool Approval Flow

```
agent-core dispatches ToolCall
    → registry.get(name) → Tool impl
        → tool.requires_approval()?
            → yes: ApprovalCallback::request_approval(call)
                → Approved: execute, emit ToolCallApproved + ToolCallCompleted
                → Denied:   skip,    emit ToolCallDenied  + ToolCallCompleted(Denied)
            → no: execute directly, emit ToolCallCompleted
```

## Event Stream

The agent emits `AgentEvent` values over a `tokio::sync::broadcast` channel (capacity 256).
Consumers call `agent.subscribe()` before `agent.run()` to avoid missing early events.

- `ThinkingStarted` / `ThinkingDone`
- `MessageDelta { text }` — streaming token
- `MessageComplete { message }` — full assistant turn
- `ToolCallRequested { call, requires_approval }`
- `ToolCallApproved { call_id }` / `ToolCallDenied { call_id }`
- `ToolCallCompleted { result }`
- `Warning { message }` — non-fatal (e.g. max iterations reached)
- `SessionError { error }` — fatal
- `SessionComplete`

## Sandbox (agent-tools)

`Sandbox` enforces path restrictions before any tool touches the filesystem:
- `check_read(path)` — resolves relative paths against `workspace_root`, blocks path traversal (`../`)
- `check_write(path)` — same, write-side
- `check_shell()` — returns error if `allow_shell` is false
- `Sandbox::new(root)` — shell disabled (read/write only)
- `Sandbox::permissive(root)` — shell + net enabled (used by CLI)

## Memory Files

- Written to `{workspace_root}/memory/*.md` by the `MemoryTool`
- Names: alphanumeric + hyphens/underscores only (sanitised before write)
- Read at session start by `load_memories()` in `agent-tools/src/impls/memory.rs`
- Injected into system prompt under `## Persistent Memory` heading
- Excluded from git via root `.gitignore` (`memory/`, `**/memory/`)

## CLI (crates/cli)

Four source files:

| File | Role |
|------|------|
| `main.rs` | `clap` arg parsing. Subcommands: `run <prompt>`, `chat`. Global flags: `--workspace`, `--model`, `--yes`. |
| `session.rs` | Builds `Agent` via `AgentBuilder`, drives the broadcast event loop, implements `run_once` and `run_chat`. |
| `approval.rs` | `TerminalApproval` — yellow approval box, reads `y/N` from stdin, denies on non-interactive pipe. |
| `display.rs` | Renders each `AgentEvent` — streaming text inline, tool calls with icon + truncated input, errors in red. |

## Docker

**Dockerfile** — two stages:
- `builder`: `rust:1.87-slim`, installs `pkg-config` + `libssl-dev`, caches deps via stub sources, then builds release binary
- `runtime`: `debian:bookworm-slim`, copies `code-agent` binary, `ENTRYPOINT ["code-agent"]`

**docker-compose.yml** services:
- `dev` — source volume-mounted builder image; base for check/test/clippy
- `check` — `cargo check`
- `test` — `cargo test`
- `clippy` — `cargo clippy -- -D warnings`
- `cli` — runtime image; mounts `$WORKSPACE` (or `$PWD`) as `/workspace`

**Cargo cache** — `cargo-cache`, `cargo-git`, `target-cache` named volumes keep rebuilds fast.

## scripts/start.sh

Wrapper around `docker compose run --rm cli` that:
1. Checks Docker is installed and `ANTHROPIC_API_KEY` is set
2. Builds the `cli` image if it doesn't exist (or `--rebuild` is passed)
3. Mounts `$WORKSPACE` (default: `$PWD`) and passes API keys through
4. Runs `chat` by default; runs `run "<prompt>"` if a positional arg is given
5. Respects `--yes` (auto-approve), `--model`, `--workspace` flags

Default `ANTHROPIC_API_KEY` fallback is set in the script for quick testing — replace with a real key.

## Adding a New LLM Provider

1. Create `crates/agent-llm/src/<name>.rs`
2. Implement `LlmProvider` trait (`name`, `complete`, `stream`)
3. Re-export from `crates/agent-llm/src/lib.rs`
4. No changes needed in `agent-core`, `agent-tools`, or `cli`

## Adding a New Tool

1. Create `crates/agent-tools/src/impls/<name>.rs`
2. Implement `Tool` trait (`schema`, `requires_approval`, `execute`)
3. Add to `mod.rs` and re-export from `lib.rs`
4. Register in `AgentBuilder::default_tool_registry()` in `agent-core/src/builder.rs`
