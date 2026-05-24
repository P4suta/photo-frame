# syntax=docker/dockerfile:1.7
# All ARG pins below are auto-bumped by Renovate.
# Look at renovate.json regexManagers for the matching rules.

# renovate: datasource=docker depName=rust versioning=docker
ARG RUST_VERSION=1.95.0
FROM rust:${RUST_VERSION}-slim-bookworm

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=always \
    RUSTUP_PERMIT_COPY_RENAME=1 \
    BUN_INSTALL=/usr/local \
    PATH=/usr/local/bin:$PATH

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates curl git pkg-config libssl-dev build-essential \
        unzip xz-utils \
 && rm -rf /var/lib/apt/lists/*

# bun (Zig-based JS runtime + package manager). Replaces Node.js + npm
# entirely: bun runs Vite, installs deps from package.json, manages the
# bun.lockb lockfile. Latest at build time per the workspace's "track
# upstream" policy.
RUN curl -fsSL https://bun.sh/install | bash \
 && bun --version

# Rust components & WASM target
RUN rustup component add clippy rustfmt \
 && rustup target add wasm32-unknown-unknown

# Cargo-installed CLI tooling — latest at image build time.
# Renovate bumps via the Cargo.toml workspace deps elsewhere; here we deliberately
# pull "latest" so each Docker rebuild picks up the newest CLI.
RUN cargo install --locked just \
 && cargo install --locked taplo-cli \
 && cargo install --locked typos-cli \
 && cargo install --locked wasm-pack \
 && cargo install --locked cargo-deny

# biome (Rust, npm-published) and lefthook (Go, npm-published). bun is
# npm-compatible so a single `bun install -g` covers both — no need to
# resurrect Node.js or chase per-tool binary release URLs.
RUN bun install -g @biomejs/biome lefthook \
 && biome --version \
 && lefthook version

# Non-root user matching host UID/GID for clean bind-mount permissions.
ARG USER_UID=1000
ARG USER_GID=1000
RUN groupadd --gid ${USER_GID} dev \
 && useradd  --uid ${USER_UID} --gid ${USER_GID} --shell /bin/bash --create-home dev \
 && mkdir -p /workspace \
 && chown -R dev:dev /workspace /home/dev

USER dev
WORKDIR /workspace

CMD ["bash"]
