# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

WezTerm is a GPU-accelerated cross-platform terminal emulator and multiplexer written in Rust. User docs live at https://wezterm.org (sources in `docs/`).

## Commands

```console
$ ./get-deps                      # install platform build deps (system packages)
$ make check                      # cargo check, plus crates needing separate checks (no_std/feature variants)
$ make build                      # builds wezterm, wezterm-gui, wezterm-mux-server, strip-ansi-escapes
$ make test                       # cargo nextest run (+ wezterm-escape-parser, which is no_std by default)
$ cargo nextest run -p term       # tests for one crate
$ cargo nextest run -p term test_name   # single test (substring filter)
$ make fmt                        # cargo +nightly fmt — CI checks with `cargo +nightly fmt --all -- --check`
$ make servedocs                  # build & live-serve docs (mkdocs via ci/build-docs.sh)
```

- Formatting requires **nightly** rustfmt (`.rustfmt.toml` uses `imports_granularity = "Module"`). Lua examples use stylua (`ci/stylua.toml`).
- `cargo test --all` also works if nextest isn't installed.
- Run the GUI in debug: `cargo run --bin wezterm-gui`. Use `wezterm-gui start --always-new-process` so mux logs aren't swallowed by an earlier background instance, and `--config-file ./test-conf.lua` to test a specific config.
- CI is a single `.github/workflows/release.yml`: pushing a `20*` tag (`ci/tag.sh && git push --tags`) or running it manually builds Linux/macOS/Windows packages via `ci/deploy.sh` and publishes a GitHub release on this fork.
- `ci/update-derived-files.sh` regenerates shell completions and doc examples from the built `target/debug/wezterm`; rerun it after changing CLI args or default key tables.

## Architecture

Cargo workspace of many crates. The layering, bottom to top:

- **Escape parsing / cells**: `vtparse` (low-level VT state machine) → `wezterm-escape-parser` (semantic escape sequences; no_std by default) → `wezterm-cell`, `wezterm-surface`. `termwiz` is a standalone terminal-UI library also used internally (and re-exports much of the above).
- **`term/`**: the core terminal model (`TerminalState`, screen, scrollback, escape handling). Windowing-system agnostic. New escape sequence support goes here; aim for xterm compatibility (https://invisible-island.net/xterm/ctlseqs/ctlseqs.html). Tests in `term/src/test/` use helpers to assert screen contents.
- **`mux/`**: multiplexer model — `Mux` singleton holding `Window`s → `Tab`s → `Pane`s (split tree via `bintree`), and `Domain`s (local, SSH, tmux control mode, remote client) that spawn panes. `localpane.rs` wraps a `term` + `pty`.
- **`codec/`**: PDU wire protocol between mux client and server. Bump `CODEC_VERSION` on backwards-incompatible changes.
- **`wezterm-mux-server-impl/`** (server side session handling) and **`wezterm-client/`** (client domain connecting to a remote mux over unix socket/SSH/TLS). `wezterm-mux-server/` is the thin binary.
- **`window/`**: platform windowing abstraction (`os/{macos,windows,x11,wayland}`), OpenGL/EGL setup, input events.
- **`wezterm-font/`**: font discovery, shaping (harfbuzz), rasterization.
- **`wezterm-gui/`**: the GUI binary. `termwindow/` is the main per-window state, event handling and rendering (OpenGL and WebGPU via `shader.wgsl`); `overlay/` holds launcher, copy mode, quick select, etc.; `inputmap.rs` resolves key bindings.
- **`wezterm/`**: the `wezterm` CLI binary (subcommands like `cli`, `imgcat`, `ssh`, `connect`), which launches `wezterm-gui` for GUI work.
- **`config/`**: the `Config` struct and Lua config loading. Config fields use `#[dynamic(default = "...")]` from `wezterm-dynamic` (a serde-like Lua⇄Rust conversion layer). `KeyAssignment` enum is in `config/src/keyassignment.rs`.
- **`lua-api-crates/`**: each crate registers a slice of the `wezterm` Lua module (`wezterm.mux`, `wezterm.time`, etc.); `luahelper/` has shared Lua glue.
- **`pty/`**, **`wezterm-ssh/`**, **`filedescriptor/`**: cross-platform PTY, SSH client (libssh/ssh2 backends), fd utilities.

## Conventions

- Commit subjects are prefixed with the affected component, e.g. `mux: ...`, `term: ...`, `wezterm-gui: ...`, `x11: ...`.
- New or changed behavior should come with docs: config options have one page each under `docs/config/lua/config/<option>.md`; Lua API objects under `docs/config/lua/`. User-visible changes get a note in `docs/changelog.md` under "Continuous/Nightly".
- Include tests for terminal behavior changes.
