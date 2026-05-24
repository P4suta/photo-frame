# syntax=docker/dockerfile:1.7
# All ARG pins below are auto-bumped by Renovate.
# Look at renovate.json regexManagers for the matching rules.

# renovate: datasource=docker depName=rust versioning=docker
ARG RUST_VERSION=1.95.0
# Debian trixie (13) ships libheif >= 1.19; bookworm's 1.15 is too old for
# the current libheif-sys (requires >= 1.18). Switching the base unlocks
# the opt-in `heif` cargo feature without compiling libheif from source.
FROM rust:${RUST_VERSION}-slim-trixie

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=always \
    RUSTUP_PERMIT_COPY_RENAME=1 \
    BUN_INSTALL=/usr/local \
    PATH=/usr/local/bin:$PATH

# ── apt with cache mount ─────────────────────────────────────────────────
# BuildKit cache mount keeps the apt package cache + lists between Docker
# builds. `rm -rf /var/lib/apt/lists/*` would defeat the cache, so we keep
# the lists inside the cache mount instead and avoid the extra cleanup
# step. The image carries zero apt cache thanks to the mount being
# external to the layer.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates curl git pkg-config libssl-dev build-essential \
        unzip xz-utils \
        libheif-dev clang

# libheif-dev backs the opt-in `heif` cargo feature on photo-frame-decode
# (HEIC / HEIF input). The default cargo build does not require it; only
# `cargo build -p photo-frame-decode --features heif` does. clang is here
# because libheif-sys runs bindgen at build time and needs libclang.

# ── bun ──────────────────────────────────────────────────────────────────
# Zig-based JS runtime + package manager. Replaces Node.js + npm entirely:
# bun runs Vite, installs deps from package.json, manages the bun.lock
# lockfile. Latest at build time per the workspace's "track upstream"
# policy. We also symlink `node` → `bun` so that JS shebang lines
# (`#!/usr/bin/env node`) inside packages installed globally by bun
# (lefthook etc.) can find a runtime.
RUN curl -fsSL https://bun.sh/install | bash \
 && bun --version \
 && ln -sf "$(which bun)" /usr/local/bin/node

# ── Rust toolchain components ────────────────────────────────────────────
RUN rustup component add clippy rustfmt \
 && rustup target add wasm32-unknown-unknown

# ── Cargo tooling via cargo-binstall ─────────────────────────────────────
# cargo-binstall fetches precompiled GitHub-release binaries instead of
# building each tool from source. Tradeoff: a one-time cost to install
# cargo-binstall itself (≈25 s), then every subsequent tool drops from
# minutes (compile) to seconds (download + extract).
#
# BuildKit cache mounts on /usr/local/cargo/{registry,git} preserve the
# downloaded crate metadata + .crate cache across Docker rebuilds so a
# Dockerfile edit doesn't force every tool to refetch.
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
        hyperfine

# ── npm-published tooling (biome + lefthook) ─────────────────────────────
# bun is npm-compatible so a single `bun install -g` covers both — no need
# to resurrect Node.js or chase per-tool binary release URLs.
RUN bun install -g @biomejs/biome lefthook \
 && biome --version \
 && lefthook version

# ── Non-root user ────────────────────────────────────────────────────────
# Matches host UID/GID for clean bind-mount permissions. ARG names are
# HOST_UID / HOST_GID (not UID / GID) because `UID` is a readonly bash
# variable on the host shell — using it as a compose arg breaks
# `docker compose build` from bash. See docker-compose.yml.
ARG HOST_UID=1000
ARG HOST_GID=1000
RUN groupadd --gid ${HOST_GID} dev \
 && useradd  --uid ${HOST_UID} --gid ${HOST_GID} --shell /bin/bash --create-home dev \
 && mkdir -p /workspace /home/dev/.cargo/registry /home/dev/.cargo/git \
 && chown -R dev:dev /workspace /home/dev

USER dev
WORKDIR /workspace

CMD ["bash"]
