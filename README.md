# qmenu

Experimental launcher with a small Quickshell frontend and a Rust daemon/CLI.

## Layout

- `frontend/` — minimal Quickshell UI
- `crates/qmenu-daemon/` — persistent provider and UI coordinator
- `crates/qmenu-cli/` — dmenu-style stdin/stdout client
- `crates/qmenu-protocol/` — shared wire types

## Run the frontend

```sh
qs -p frontend/shell.qml
```

## Check Rust crates

```sh
cargo check
```

The daemon, CLI, and transport are intentionally stubs while the interaction model is explored.
