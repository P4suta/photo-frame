# syntax=docker/dockerfile:1.7
# All ARG pins below are auto-bumped by Renovate.
# Look at renovate.json regexManagers for the matching rules.

# renovate: datasource=docker depName=rust versioning=docker
ARG RUST_VERSION=1.95.0
# Debian trixie (13) ships libheif >= 1.19; bookworm's 1.15 is too old
# for the current libheif-sys (requires >= 1.18). Switching the base
# unlocks the opt-in `heif` cargo feature without compiling libheif
# from source.

# ─── chef-base ───────────────────────────────────────────────────────────
# Shared base layer with cargo-chef installed, used by every stage in the
# release-image pipeline (planner → cacher → builder → runtime). Pulling
# this out as a separate stage means cargo-chef itself only compiles once
# per Docker rebuild, not once per stage.
FROM rust:${RUST_VERSION}-slim-trixie AS chef-base

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=always

# cargo-binstall + cargo-chef. Installing binstall once (~25 s compile)
# and using it to fetch cargo-chef as a precompiled binary saves the
# 2-3 minute cargo-chef-from-source compile on every Dockerfile change
# that invalidates this layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo install --locked cargo-binstall \
 && cargo binstall --no-confirm --no-symlinks cargo-chef

# ─── planner ─────────────────────────────────────────────────────────────
# Walks the workspace and emits `recipe.json`, a normalised description
# of which crates need to compile. recipe.json is essentially Cargo.lock
# minus the project's own crates — Docker's layer cache compares it byte
# for byte, so the cacher stage below stays cached as long as
# Cargo.{toml,lock} and dependencies don't change.
FROM chef-base AS planner
WORKDIR /workspace
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ─── cacher ──────────────────────────────────────────────────────────────
# Compiles **dependencies only** from the recipe. Workspace source isn't
# copied in here — the layer that COPYs source lives in `builder` below.
# Editing any photo-frame .rs file therefore does not invalidate this
# (large, slow) dependency-compile layer.
FROM chef-base AS cacher

# libheif-dev + clang are needed because the heif feature on
# photo-frame-decode pulls in libheif-sys (which runs bindgen at build
# time and needs both). The runtime stage decides per-build whether to
# include them in the final image; here in cacher we always compile the
# full dep graph so a `--build-arg ENABLE_HEIF=1` switch wouldn't change
# the cooked layer.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev build-essential libheif-dev clang

WORKDIR /workspace
COPY --from=planner /workspace/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json --workspace

# ─── builder ─────────────────────────────────────────────────────────────
# Adds the actual workspace source on top of cacher's compiled
# dependency layer and builds the CLI. Touching only project source
# (a frequent operation) reuses the dep cache from cacher.
FROM cacher AS builder
WORKDIR /workspace
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release -p photo-frame-cli

# ─── runtime ─────────────────────────────────────────────────────────────
# Thin distribution image — just the CLI binary + ca-certificates +
# tzdata. Use this with: `docker build --target runtime -t photo-frame .`
# Image size is roughly the binary (~10-15 MB) + base layer (~80 MB).
FROM debian:trixie-slim AS runtime

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates tzdata

COPY --from=builder /workspace/target/release/photo-frame /usr/local/bin/photo-frame
ENTRYPOINT ["photo-frame"]
CMD ["--help"]

# ─── dev ─────────────────────────────────────────────────────────────────
# Development image used by docker-compose. Has the full Rust toolchain,
# clippy/rustfmt, the WASM target, every workspace tool (just, taplo,
# typos, wasm-pack, cargo-deny, cargo-nextest, hyperfine, cargo-chef,
# cargo-binstall), bun + biome + lefthook, plus libheif-dev for the
# `heif` feature.
FROM rust:${RUST_VERSION}-slim-trixie AS dev

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=always \
    RUSTUP_PERMIT_COPY_RENAME=1 \
    BUN_INSTALL=/usr/local \
    PATH=/usr/local/bin:$PATH

# apt: cache mounts keep the package cache + lists across rebuilds.
# valgrind is the backend `iai-callgrind-runner` shells out to for the
# runtime-bench icount harness; without it `just bench-icount` panics
# at runner startup with a "valgrind not on PATH" diagnostic.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates curl git pkg-config libssl-dev build-essential \
        unzip xz-utils \
        libheif-dev clang \
        valgrind

# bun (Zig-based JS runtime + package manager). Symlinking `node` → `bun`
# so npm-published packages with `#!/usr/bin/env node` shebangs (lefthook
# etc.) can find a runtime.
RUN curl -fsSL https://bun.sh/install | bash \
 && bun --version \
 && ln -sf "$(which bun)" /usr/local/bin/node

# Rust toolchain components.
RUN rustup component add clippy rustfmt \
 && rustup target add wasm32-unknown-unknown

# Phase F2 — nightly toolchain for `crates/photo-frame-wasm/` only.
# rust-toolchain.toml inside that crate pins nightly-2026-04-01; we
# install it ahead of time so `just wasm-build` doesn't pay the
# 60-second download on first invocation. rust-src is the std
# source needed for `-Z build-std`, which `wasm-bindgen-rayon`
# requires (see crates/photo-frame-wasm/.cargo/config.toml).
RUN rustup install nightly-2026-04-01 --profile minimal \
 && rustup component add --toolchain nightly-2026-04-01 rust-src rustfmt clippy \
 && rustup target  add --toolchain nightly-2026-04-01 wasm32-unknown-unknown

# Cargo tooling: install cargo-binstall once from source, then use it to
# pull every other tool as a precompiled GitHub-release binary. Drops
# minutes (compile) to seconds (download + extract) per tool. cargo-chef
# lives here too so developers can rebuild the runtime image from inside
# the dev container.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo install --locked cargo-binstall

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo binstall --no-confirm --no-symlinks --force \
        just \
        taplo-cli \
        typos-cli \
        wasm-pack \
        cargo-deny \
        cargo-nextest \
        hyperfine \
        cargo-chef \
        samply \
        inferno \
        iai-callgrind-runner

# npm-published tooling.
RUN bun install -g @biomejs/biome lefthook \
 && biome --version \
 && lefthook version

# Non-root user matching host UID/GID for clean bind-mount permissions.
# ARG names are HOST_UID / HOST_GID (not UID / GID) because `UID` is a
# readonly bash variable on the host shell — using it as a compose arg
# breaks `docker compose build` from bash. See docker-compose.yml.
ARG HOST_UID=1000
ARG HOST_GID=1000
RUN groupadd --gid ${HOST_GID} dev \
 && useradd  --uid ${HOST_UID} --gid ${HOST_GID} --shell /bin/bash --create-home dev \
 && mkdir -p /workspace /home/dev/.cargo/registry /home/dev/.cargo/git \
 && chown -R dev:dev /workspace /home/dev

USER dev
WORKDIR /workspace

CMD ["bash"]
