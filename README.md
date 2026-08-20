# Yobu

A Quickshell launcher backed by a generic Rust data-provider daemon.

`yobud` owns data, actions, search indexes, and persisted state. Quickshell independently owns presentation and visibility. The small `yobu` CLI wraps Quickshell IPC and supports interactive piped menus.

## Components

- `src/daemon/` — newline-delimited JSON broker over a Unix socket
- `src/providers/` — applications, calculator, state, CPU, and ephemeral menu providers
- `src/bin/yobud.rs` — data-provider daemon
- `src/bin/yobu.rs` — Quickshell IPC and piped-menu wrapper
- `frontend/DaemonConnection.qml` — shared reconnecting daemon connection
- `frontend/*Module.qml` — provider-specific frontend state
- `frontend/shell.qml` — launcher composition and `yobu` IPC target

## Run

Start the daemon and frontend independently:

```sh
yobud
qs -p ~/.config/quickshell/yobu/shell.qml
```

Both use `$YOBU_SOCKET` when set, otherwise `$XDG_RUNTIME_DIR/yobu.sock`.

Control the launcher through the CLI:

```sh
yobu              # toggle
yobu --show
yobu --hide
yobu --toggle
```

Or call Quickshell directly:

```sh
qs ipc -p ~/.config/quickshell/yobu/shell.qml call yobu toggle
qs ipc -p ~/.config/quickshell/yobu/shell.qml call yobu setVisible true
qs ipc -p ~/.config/quickshell/yobu/shell.qml call yobu refresh
```

## Piped menus

Plain lines become searchable menu entries. The selected value is written to stdout; cancellation exits with status 1.

```sh
printf '%s\n' Alpha Beta Gamma | yobu "Pick one"
```

Structured JSON lines can include `id`, `label`, `value`, and optional `description`, `icon`, and `preview` fields:

```sh
printf '%s\n' \
  '{"id":"image","label":"Screenshot","value":"chosen","preview":"file:///tmp/example.png"}' \
  | yobu --json-lines "Pick one"
```

Clipboard history has a dedicated adapter with cached image previews:

```sh
selection="$(yobu --cliphist)" \
  && printf '%s\n' "$selection" | cliphist decode | wl-copy
```

## Features

- Fuzzy application and piped-entry search
- Persisted frequency/recency application ranking
- Automatic debounced refresh when desktop-entry directories change
- Calculator results that replace application matches and copy on acceptance
- Per-screen persisted launcher position and configurable fractional or pixel dimensions
- Ephemeral daemon-backed piped-menu sessions
- Selected-item image previews, including cliphist images
- Quickshell reconnect and IPC show/hide/toggle control

State is stored in `$XDG_STATE_HOME/yobu/state.toml` (normally `~/.local/state/yobu/state.toml`). Configuration is read from `$XDG_CONFIG_HOME/yobu/config.toml`.

## Development

Use the same alternate socket for isolated processes:

```sh
YOBU_SOCKET=/tmp/yobu-dev.sock cargo run --bin yobud
YOBU_SOCKET=/tmp/yobu-dev.sock qs -p "$PWD/frontend/shell.qml"
```

Verify the project with:

```sh
cargo check --all-targets
cargo test --lib
```

Run Criterion benchmarks with:

```sh
cargo bench --bench performance
```
