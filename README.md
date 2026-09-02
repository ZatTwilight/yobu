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

Start `yobud` and a Quickshell configuration containing the launcher components. The `yobu` CLI targets the `bar` configuration by default; override it with `$YOBU_QUICKSHELL_CONFIG`.

```sh
yobud
quickshell -c bar
```

For daily use, install the binaries and link the repository's user service
(run from a stable checkout path):

```sh
cargo install --path .
mkdir -p ~/.config/systemd/user
ln -s "$PWD/systemd/yobud.service" ~/.config/systemd/user/yobud.service
systemctl --user daemon-reload
systemctl --user enable --now yobud.service
```

Remove direct yobud startup from compositor autostart to avoid duplicate daemons.
The service runs `~/.cargo/bin/yobud` and follows `graphical-session.target`.
Adjust `ExecStart` if using a custom Cargo installation directory.

After code changes:

```sh
cargo test --lib && cargo install --path . && systemctl --user restart yobud.service
```

After unit changes, run `systemctl --user daemon-reload` before restarting.
Inspect status with `systemctl --user status yobud.service` and logs with
`journalctl --user -u yobud.service -n 30`.

The daemon and embedded launcher use `$YOBU_SOCKET` when set, otherwise `$XDG_RUNTIME_DIR/yobu.sock`.

Control the launcher through the CLI:

```sh
yobu              # toggle
yobu --show
yobu --hide
yobu --toggle
```

Or call Quickshell directly:

```sh
qs ipc -c bar call yobu toggle
qs ipc -c bar call yobu setVisible true
qs ipc -c bar call yobu refresh
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

Clipboard history has a dedicated adapter with cached image previews. It displays the menu, decodes the selected record, and copies it back to the Wayland clipboard:

```sh
yobu --cliphist
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

## Application ownership

Application launches use independent systemd user services, not yobud's cgroup.
Configure the backend in `~/.config/yobu/config.toml`:

```toml
[launch]
backend = "auto" # auto (default), uwsm, or systemd
```

`auto` prefers `uwsm app -t service` when `uwsm check is-active` succeeds;
otherwise it requires a reachable systemd user manager and uses `systemd-run`.
Explicit backends fail if unavailable. Configuration is read on each launch;
invalid launch configuration is an error. Failed launches are never retried
through another backend or direct spawning. Helpers are reaped and have a
10-second timeout; a timeout may mean the application already started.

UWSM receives the desktop-entry path and handles desktop launch semantics,
including terminal entries. Keep its systemd activation environment current.
The generic systemd backend executes the parsed `Exec` arguments literally,
without shell or environment expansion, and rejects terminal entries. It uses
`app.slice`, the user manager's environment, and selected desktop-session
variables from yobud; it does not implement additional desktop-entry semantics
such as `Path` or D-Bus activation.

No memory limits, OOM immunity, or global OOM settings are changed. Existing
application instances may receive launches without creating new processes;
workloads started inside terminals still need their own isolation if desired.

## AI account limits

The `ai` provider publishes Codex and OpenCode Go account usage limits and reset
timestamps (`resetsAt`) through `ai.codex`, `ai.opencode`, and `ai.status`.
Consumers can calculate time remaining from those timestamps. Collection runs
every 300 seconds by default (`[ai] poll_secs`), or on `ai.refresh`.
It does not read session histories or message databases, or collect token counts.
Records expose `hasLocalStats: false`; historical statistics fields are omitted.

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
