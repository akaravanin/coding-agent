#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# start.sh — build and launch code-agent CLI
#
# Usage:
#   ./scripts/start.sh                           # interactive chat (anthropic)
#   ./scripts/start.sh "fix the failing test"    # one-shot run
#   ./scripts/start.sh --provider gemini "..."   # use Gemini
#   ./scripts/start.sh --workspace /my/proj      # different workspace
#   ./scripts/start.sh --yes "add docs"          # auto-approve all tool calls
#   ./scripts/start.sh --rebuild                 # force image rebuild
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load keys from .params if present (gitignored local config)
PARAMS_FILE="$REPO_ROOT/.params"
if [[ -f "$PARAMS_FILE" ]]; then
  # shellcheck disable=SC1090
  set -a; source "$PARAMS_FILE"; set +a
fi

# --- Defaults ---
WORKSPACE="${WORKSPACE:-$(pwd)}"
PROVIDER="${CODE_AGENT_PROVIDER:-anthropic}"
MODEL=""
AUTO_APPROVE=""
REBUILD=false
PROMPT=""

# --- Colours ---
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

info()  { echo -e "${CYAN}${BOLD}[code-agent]${RESET} $*"; }
warn()  { echo -e "${YELLOW}[warn]${RESET} $*" >&2; }
error() { echo -e "${RED}[error]${RESET} $*" >&2; exit 1; }

# --- Parse args ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace|-w)
      WORKSPACE="$2"; shift 2 ;;
    --provider|-p)
      PROVIDER="$2"; shift 2 ;;
    --model)
      MODEL="$2"; shift 2 ;;
    --yes|-y)
      AUTO_APPROVE="--yes"; shift ;;
    --rebuild)
      REBUILD=true; shift ;;
    --help|-h)
      sed -n '/^# Usage:/,/^# ---/p' "$0" | sed 's/^# \?//'
      exit 0 ;;
    -*)
      error "Unknown option: $1 (run with --help)" ;;
    *)
      PROMPT="$1"; shift ;;
  esac
done

# --- Set default model per provider ---
if [[ -z "$MODEL" ]]; then
  case "$PROVIDER" in
    gemini)    MODEL="gemini-2.0-flash" ;;
    anthropic) MODEL="claude-sonnet-4-6" ;;
    *)         error "Unknown provider: $PROVIDER (use 'anthropic' or 'gemini')" ;;
  esac
fi

# --- Checks ---
command -v docker >/dev/null 2>&1 || error "Docker is not installed or not in PATH."

case "$PROVIDER" in
  anthropic)
    [[ -z "${ANTHROPIC_API_KEY:-}" ]] && error "ANTHROPIC_API_KEY is not set."
    ;;
  gemini)
    [[ -z "${GEMINI_API_KEY:-}" ]] && error "GEMINI_API_KEY is not set."
    ;;
esac

[[ -d "$WORKSPACE" ]] || error "Workspace directory does not exist: $WORKSPACE"
WORKSPACE="$(cd "$WORKSPACE" && pwd)"

# --- Build image if needed ---
IMAGE_NAME="code-agent:latest"
COMPOSE_FILE="$REPO_ROOT/docker-compose.yml"

needs_build() {
  ! docker image inspect "$IMAGE_NAME" >/dev/null 2>&1
}

if $REBUILD || needs_build; then
  info "Building Docker image (this takes a minute the first time)…"
  docker compose -f "$COMPOSE_FILE" build cli
  info "Build complete."
else
  info "Image ${IMAGE_NAME} found. Skipping build (use --rebuild to force)."
fi

# --- Compose the docker run command ---
DOCKER_ARGS=(
  "--rm"
  "--interactive"
  "--tty"
  "--volume" "${WORKSPACE}:/workspace"
  "--workdir" "/workspace"
  "--env" "ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}"
  "--env" "GEMINI_API_KEY=${GEMINI_API_KEY}"
)

AGENT_ARGS=("--provider" "$PROVIDER" "--model" "$MODEL")
[[ -n "$AUTO_APPROVE" ]] && AGENT_ARGS+=("--yes")

if [[ -n "$PROMPT" ]]; then
  AGENT_ARGS+=("run" "$PROMPT")
else
  AGENT_ARGS+=("chat")
fi

# --- Banner ---
echo ""
echo -e "${BOLD}  code-agent${RESET}"
echo -e "  workspace : ${CYAN}${WORKSPACE}${RESET}"
echo -e "  provider  : ${CYAN}${PROVIDER}${RESET}"
echo -e "  model     : ${CYAN}${MODEL}${RESET}"
echo -e "  mode      : ${CYAN}$( [[ -n "$PROMPT" ]] && echo "run" || echo "chat" )${RESET}"
[[ -n "$AUTO_APPROVE" ]] && echo -e "  approval  : ${YELLOW}auto-approve (--yes)${RESET}"
echo ""

# --- Launch ---
docker run "${DOCKER_ARGS[@]}" "$IMAGE_NAME" "${AGENT_ARGS[@]}"
