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

## Calculator results

The native application launcher evaluates non-empty queries with the MIT-licensed Numby engine. Calculations—including arithmetic, percentages, units, currencies, dates, and timezone queries—replace application matches with their formatted result. Queries that are not calculations continue through application search. Piped menus do not invoke the calculator.

The calculator runs directly inside the Rust daemon and keeps Numby's variable and history state for the daemon's lifetime.

The launcher can be dragged by its top handle. Its normalized position is saved per monitor in `$XDG_STATE_HOME/qmenu/state.toml` (normally `~/.local/state/qmenu/state.toml`).

Optional UI configuration is loaded from `$XDG_CONFIG_HOME/qmenu/config.toml`:

```toml
width = 600
height = 420
max_results = 50
```

See `config.example.toml`. The daemon rereads this file whenever the launcher opens, so changes apply on the next launch without restarting the daemon.

## Performance benchmarks

Run the Criterion benchmarks in release mode:

```sh
cargo bench -p qmenu-daemon --bench performance
```

They cover desktop-entry discovery, JSON serialization at several catalog sizes, and a Rust equivalent of the frontend's current substring search. Criterion stores detailed reports under `target/criterion/`.

With the daemon running, measure the real request-to-first-animation-frame path:

```sh
cargo run --release -q -p qmenu -- --benchmark-display 20
```

The display benchmark includes socket connection, request serialization, daemon forwarding, frontend JSON/model setup, and the first frame queued for presentation after the launcher becomes visible. It excludes daemon startup and CLI process startup. The first sample includes Quickshell startup when no frontend is already connected; the command reports warm and steady-state statistics separately. Iterations have an unmeasured 100 ms settling period so the compositor can process each unmap before the next request.

To include CLI process startup, build first and time a single warm display externally:

```sh
cargo build --release -p qmenu
hyperfine --warmup 3 'target/release/qmenu --benchmark-display 1'
```

The benchmark closes the launcher automatically after each frame.

## Check the project

```sh
cargo check
```

This is an initial vertical slice. The protocol currently uses newline-delimited JSON and the frontend performs simple substring filtering.
