# qmenu

Experimental launcher with a small Quickshell frontend and a Rust daemon/CLI.

## Layout

- `frontend/` — minimal Quickshell UI
- `crates/qmenu-daemon/` — persistent provider and UI coordinator
- `crates/qmenu-cli/` — dmenu-style stdin/stdout client
- `crates/qmenu-protocol/` — shared wire types

## Try the piped menu

Start the daemon from the repository root so it can locate `frontend/shell.qml`:

```sh
cargo run -p qmenu-daemon
```

Then, from another terminal, open the application launcher:

```sh
cargo run -q -p qmenu
```

Or provide piped entries, which replaces the application list for that invocation:

```sh
printf '%s\n' Alpha Beta Gamma | cargo run -q -p qmenu -- "Pick one"
```

The daemon starts Quickshell, forwards the entries, and returns piped selections on stdout. Application selections are launched directly. Escape or clicking outside exits the client with status 1.

Set `QMENU_SOCKET` to override the Unix socket path and `QMENU_FRONTEND` to override the QML entrypoint.

The launcher can be dragged by its top handle. Its normalized position is saved per monitor in `$XDG_STATE_HOME/qmenu/state.toml` (normally `~/.local/state/qmenu/state.toml`).

Optional UI configuration is loaded from `$XDG_CONFIG_HOME/qmenu/config.toml`:

```toml
width = 600
height = 420
max_results = 50
```

See `config.example.toml`.

## Check the project

```sh
cargo check
```

This is an initial vertical slice. The protocol currently uses newline-delimited JSON and the frontend performs simple substring filtering.
