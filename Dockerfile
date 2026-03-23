FROM rust:1.87-slim AS builder

WORKDIR /app

# Install system deps needed by reqwest (TLS)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies: copy manifests first, then build a dummy main
COPY Cargo.toml ./
COPY crates/agent-protocol/Cargo.toml  crates/agent-protocol/Cargo.toml
COPY crates/agent-llm/Cargo.toml       crates/agent-llm/Cargo.toml
COPY crates/agent-tools/Cargo.toml     crates/agent-tools/Cargo.toml
COPY crates/agent-context/Cargo.toml   crates/agent-context/Cargo.toml
COPY crates/agent-core/Cargo.toml      crates/agent-core/Cargo.toml
COPY crates/cli/Cargo.toml             crates/cli/Cargo.toml

# Stub src files so cargo fetch/check can resolve workspace
RUN for crate in agent-protocol agent-llm agent-tools agent-context agent-core; do \
      mkdir -p crates/$crate/src && \
      echo "// stub" > crates/$crate/src/lib.rs; \
    done && \
    mkdir -p crates/cli/src && \
    echo 'fn main() {}' > crates/cli/src/main.rs

RUN cargo fetch

# Now copy real source and build
COPY . .

RUN cargo build --release

# --- Runtime image ---
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    bash \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

COPY --from=builder /app/target/release/code-agent /usr/local/bin/code-agent

ENTRYPOINT ["code-agent"]
CMD ["chat"]
