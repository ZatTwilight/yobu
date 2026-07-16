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

Then, from another terminal:

```sh
printf '%s\n' Alpha Beta Gamma | cargo run -q -p qmenu -- "Pick one"
```

The daemon starts Quickshell, forwards the entries, and returns the selected value on stdout. Escape or clicking outside exits the client with status 1.

Set `QMENU_SOCKET` to override the Unix socket path and `QMENU_FRONTEND` to override the QML entrypoint.

## Check the project

```sh
cargo check
```

This is an initial vertical slice. The protocol currently uses newline-delimited JSON and the frontend performs simple substring filtering.
