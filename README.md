# yobu

Experimental launcher with a small Quickshell frontend and a Rust daemon/CLI.

## Layout

- `frontend/` — minimal Quickshell UI
- `src/bin/` — `yobu` CLI and `yobud` daemon entry points
- `src/` — shared protocol types and provider logic

## Install

```sh
cargo install --path .
```

This installs both `yobu` and `yobud`.

## Try the piped menu

Start the daemon from the repository root so it can locate `frontend/shell.qml`:

```sh
cargo run -p yobu --bin yobud
```

Then, from another terminal, open the application launcher:

```sh
cargo run -q -p yobu --bin yobu
```

Or provide piped entries, which replaces the application list for that invocation:

```sh
printf '%s\n' Alpha Beta Gamma | cargo run -q -p yobu --bin yobu -- "Pick one"
```

The daemon starts Quickshell, forwards the entries, and returns piped selections on stdout. Application selections are launched directly. Escape or clicking outside exits the client with status 1.

The daemon bundles its Quickshell shell and default launcher view, so installed builds do not depend on the repository's `frontend/` directory. To customize the launcher, create a copy of the default view:

```sh
yobu --init-view
```

This writes `$XDG_CONFIG_HOME/yobu/LauncherView.qml` (normally `~/.config/yobu/LauncherView.qml`). The custom view is loaded the next time Quickshell starts; if it cannot be loaded, yobu falls back to the bundled view. Remove the file to restore the bundled default.

Set `YOBU_SOCKET` to override the Unix socket path, `YOBU_LAUNCHER_VIEW` to load a view from another path, or `YOBU_FRONTEND` to replace the complete QML shell.

## Calculator results

The native application launcher evaluates non-empty queries with the MIT-licensed Numby engine. Calculations—including arithmetic, percentages, units, currencies, dates, and timezone queries—replace application matches with their formatted result. Queries that are not calculations continue through application search. Piped menus do not invoke the calculator.

The calculator runs directly inside the Rust daemon and keeps Numby's variable and history state for the daemon's lifetime.

The launcher can be dragged by its top handle. Its normalized position is saved per monitor in `$XDG_STATE_HOME/yobu/state.toml` (normally `~/.local/state/yobu/state.toml`).

Optional UI configuration is loaded from `$XDG_CONFIG_HOME/yobu/config.toml`:

```toml
width = 600
height = 420
max_results = 50
```

See `config.example.toml`. The daemon rereads this file whenever the launcher opens, so changes apply on the next launch without restarting the daemon.

## Performance benchmarks

Run the Criterion benchmarks in release mode:

```sh
cargo bench -p yobu --bench performance
```

They cover desktop-entry discovery, JSON serialization at several catalog sizes, and a Rust equivalent of the frontend's current substring search. Criterion stores detailed reports under `target/criterion/`.

With the daemon running, measure the real request-to-first-animation-frame path:

```sh
cargo run --release -q -p yobu --bin yobu -- --benchmark-display 20
```

The display benchmark includes socket connection, request serialization, daemon forwarding, frontend JSON/model setup, and the first frame queued for presentation after the launcher becomes visible. It excludes daemon startup and CLI process startup. The first sample includes Quickshell startup when no frontend is already connected; the command reports warm and steady-state statistics separately. Iterations have an unmeasured 100 ms settling period so the compositor can process each unmap before the next request.

To include CLI process startup, build first and time a single warm display externally:

```sh
cargo build --release -p yobu --bins
hyperfine --warmup 3 'target/release/yobu --benchmark-display 1'
```

The benchmark closes the launcher automatically after each frame.

## Check the project

```sh
cargo check
```

This is an initial vertical slice. The protocol currently uses newline-delimited JSON and the frontend performs simple substring filtering.
