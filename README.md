# photo-frame

A Liit-style photo framing tool. Adds a clean white border to your JPEG/PNG photographs and bakes in a minimal, stylish EXIF caption — all proportioned by the golden ratio.

- **Core**: Pure Rust (`photo-frame-core`).
- **Frontends**: CLI (`photo-frame-cli`) and WASM (`photo-frame-wasm`, browser-only).
- **Inputs**: JPEG, PNG.
- **Outputs**: JPEG.
- **Status**: pre-release.

## Quick start (Docker)

The entire dev environment lives in Docker. You only need `docker` and `docker compose`.

```sh
docker compose build
docker compose run --rm dev just hooks  # install lefthook git hooks (once)
docker compose run --rm dev just ci     # fmt + lint + test + wasm-build
```

For interactive development:

```sh
docker compose run --rm --service-ports dev
# inside the container:
just              # list recipes
just wasm-dev     # serve the SPA on :5173
```

VS Code users: open the folder and "Reopen in Container" — `.devcontainer/` is configured.

## Project surface (`justfile`)

```
just ci             full CI suite (fmt-check + lint + test + wasm-build)
just fmt            format everything in place
just lint           run all linters with -D warnings
just test           run all tests
just wasm-build     build the WASM artifact
just wasm-dev       run the SPA dev server
just run <args>     run the CLI binary
```

## CLI usage

```sh
just run path/to/photo.jpg -o /tmp/out.jpg
```

## Architecture

See [`crates/photo-frame-core/src/lib.rs`](crates/photo-frame-core/src/lib.rs) for the public surface and the pipeline contract. The processing order — `decode → orientation-normalize → exif → geometry → frame → encode` — is fixed; the orientation step in particular must run before any visual processing, otherwise Nikon-Z-style portrait shots come out rotated.

The golden ratio geometry is expressed as a nested set of φⁿ scalings off a single `side` quantum. See [`geometry.rs`](crates/photo-frame-core/src/geometry.rs).

### Cargo dep purity

The CLI and WASM binaries (and the entire transitive Cargo dep tree) are **100% Pure Rust**: no C compilation, no `*-sys` FFI bridges to native libraries, no `pkg-config` / `cmake` / `bindgen` in the build path. This is enforced by `cargo-deny` via [`deny.toml`](deny.toml) and gated through `just lint-deps` (run from `just lint` and `just ci`). Adding a transitive dep that pulls `cc`, `bindgen`, `pkg-config`, `vcpkg`, `cmake`, `autotools`, or `meson` will fail CI by design.

## Licenses

- Code: dual-licensed under MIT (`LICENSE-MIT`) **or** Apache-2.0 (`LICENSE-APACHE`) at your option.
- Bundled font: **[Geist Sans](https://github.com/vercel/geist-font)** v1.7.1 by Vercel in collaboration with [basement.studio](https://basement.studio/), licensed under the **SIL Open Font License 1.1**. The font is included verbatim — file names, internal name table, and accompanying attribution files are unchanged from the upstream release. See `crates/photo-frame-core/assets/fonts/Geist/`:
  - `Geist-Regular.otf`, `Geist-Medium.otf` — the font binaries
  - `LICENSE.txt`, `OFL.txt` — license text
  - `AUTHORS.txt`, `CONTRIBUTORS.txt` — full author and contributor lists
  These files are also copied into the published web bundle so end users receive the attribution.
