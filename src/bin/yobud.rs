use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    thread,
};
use yobu::{
    apps::Applications,
    calculator::{Calculation, Calculator},
    config::{self, SavedState},
    protocol::{Entry, Message, Role},
    search::SearchIndex,
};

struct State {
    frontend: Option<UnixStream>,
    client: Option<UnixStream>,
    pending: Option<Message>,
    frontend_starting: bool,
    applications: Applications,
    active_entries: SearchIndex,
    native_mode: bool,
    calculator: Option<Calculator>,
    saved: SavedState,
    ui_config: yobu::protocol::UiConfig,
}

fn socket_path() -> PathBuf {
    env::var_os("YOBU_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
                .join("yobu.sock")
        })
}

fn send(stream: &mut UnixStream, message: &Message) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

const BUNDLED_SHELL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/shell.qml"));
const BUNDLED_VIEW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/frontend/LauncherView.qml"
));

fn spawn_frontend() -> io::Result<()> {
    let runtime_dir =
        PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into())).join("yobu");
    fs::create_dir_all(&runtime_dir)?;

    let bundled_shell = runtime_dir.join("shell.qml");
    let bundled_view = runtime_dir.join("LauncherView.qml");
    fs::write(&bundled_shell, BUNDLED_SHELL)?;
    fs::write(&bundled_view, BUNDLED_VIEW)?;

    let frontend = env::var_os("YOBU_FRONTEND")
        .map(PathBuf::from)
        .unwrap_or(bundled_shell);
    let custom_view = env::var_os("YOBU_LAUNCHER_VIEW")
        .map(PathBuf::from)
        .unwrap_or_else(|| config::config_home().join("yobu/LauncherView.qml"));

    let mut command = Command::new("qs");
    command.arg("-p").arg(frontend);
    if custom_view.is_file() {
        command.env("YOBU_LAUNCHER_VIEW", custom_view);
    }
    command.spawn()?;
    Ok(())
}

fn handle_connection(stream: UnixStream, state: Arc<Mutex<State>>) -> io::Result<()> {
    let mut lines = BufReader::new(stream.try_clone()?).lines();
    let Some(line) = lines.next() else {
        return Ok(());
    };
    let hello: Message = serde_json::from_str(&line?)?;

    match hello {
        Message::Hello {
            role: Role::Frontend,
        } => handle_frontend(stream, lines, state)?,
        Message::Hello { role: Role::Client } => handle_client(stream, &mut lines, state)?,
        _ => {}
    }

    Ok(())
}

fn handle_frontend(
    stream: UnixStream,
    lines: impl Iterator<Item = io::Result<String>>,
    state: Arc<Mutex<State>>,
) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    {
        let mut shared = state.lock().unwrap();
        shared.frontend = Some(writer.try_clone()?);
        shared.frontend_starting = false;
        if let Some(message) = shared.pending.take() {
            send(&mut writer, &message)?;
        }
    }

    for line in lines {
        let message: Message = serde_json::from_str(&line?)?;
        match message {
            Message::Selected { ref value } => {
                let mut shared = state.lock().unwrap();
                if value.starts_with("app:") {
                    match shared.applications.launch(value) {
                        Ok(()) => {
                            shared.saved.record_application(value);
                            if let Err(error) = config::save_state(&shared.saved) {
                                eprintln!("failed to save application recency: {error}");
                            }
                        }
                        Err(error) => {
                            if let Some(client) = shared.client.as_mut() {
                                send(
                                    client,
                                    &Message::Error {
                                        message: error.to_string(),
                                    },
                                )?;
                            }
                        }
                    }
                }
                if let Some(mut client) = shared.client.take() {
                    send(&mut client, &message)?;
                }
            }
            Message::Search { query } => {
                let mut shared = state.lock().unwrap();
                let calculation = if shared.native_mode {
                    shared
                        .calculator
                        .as_mut()
                        .and_then(|calculator| calculator.calculate(&query))
                } else {
                    None
                };
                let entries = match calculation {
                    Some(calculation) => vec![calculation_entry(&query, calculation)],
                    None => shared.active_entries.search(
                        &query,
                        shared.ui_config.max_results,
                        &shared.saved.recent_applications,
                    ),
                };
                send(&mut writer, &Message::SearchResults { query, entries })?;
            }
            Message::Presented | Message::Cancelled => {
                let mut shared = state.lock().unwrap();
                if let Some(mut client) = shared.client.take() {
                    send(&mut client, &message)?;
                }
            }
            Message::Position { screen, x, y } => {
                let mut shared = state.lock().unwrap();
                shared
                    .saved
                    .positions
                    .insert(screen, yobu::protocol::Position { x, y });
                if let Err(error) = config::save_state(&shared.saved) {
                    eprintln!("failed to save position: {error}");
                }
            }
            _ => {}
        }
    }

    let mut shared = state.lock().unwrap();
    shared.frontend = None;
    shared.frontend_starting = false;
    Ok(())
}

fn handle_client(
    stream: UnixStream,
    lines: &mut impl Iterator<Item = io::Result<String>>,
    state: Arc<Mutex<State>>,
) -> io::Result<()> {
    let Some(line) = lines.next() else {
        return Ok(());
    };
    let request: Message = serde_json::from_str(&line?)?;
    let Message::Show {
        prompt,
        entries,
        benchmark,
        ..
    } = request
    else {
        return Ok(());
    };

    let (message, should_spawn) = {
        let mut shared = state.lock().unwrap();
        // The daemon is long-lived, so reload UI settings for every presentation.
        // This makes config edits take effect the next time the launcher opens.
        shared.ui_config = config::load_config();
        shared.native_mode = entries.is_empty();
        let entries = if shared.native_mode {
            shared.applications.entries.clone()
        } else {
            entries
        };
        shared.active_entries = SearchIndex::new(entries);
        let initial_entries = shared.active_entries.search(
            "",
            shared.ui_config.max_results,
            &shared.saved.recent_applications,
        );
        let message = Message::Show {
            prompt,
            entries: initial_entries,
            positions: shared.saved.positions.clone(),
            config: shared.ui_config.clone(),
            benchmark,
        };
        shared.client = Some(stream.try_clone()?);

        if let Some(frontend) = shared.frontend.as_mut() {
            send(frontend, &message)?;
            (message, false)
        } else {
            shared.pending = Some(message.clone());
            let spawn = !shared.frontend_starting;
            shared.frontend_starting = true;
            (message, spawn)
        }
    };
    drop(message);

    if should_spawn && let Err(error) = spawn_frontend() {
        let mut shared = state.lock().unwrap();
        shared.frontend_starting = false;
        if let Some(mut client) = shared.client.take() {
            let _ = send(
                &mut client,
                &Message::Error {
                    message: error.to_string(),
                },
            );
        }
    }

    Ok(())
}

fn calculation_entry(query: &str, calculation: Calculation) -> Entry {
    Entry {
        id: "calculation".into(),
        label: calculation.value.clone(),
        description: Some(format!("{} · {query}", calculation.kind)),
        icon: Some("accessories-calculator".into()),
        value: calculation.value,
    }
}

fn main() -> io::Result<()> {
    let path = socket_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    println!("yobu daemon listening on {}", path.display());

    let calculator = match Calculator::new() {
        Ok(calculator) => Some(calculator),
        Err(error) => {
            eprintln!("Numby unavailable; calculator results disabled: {error}");
            None
        }
    };

    let state = Arc::new(Mutex::new(State {
        frontend: None,
        client: None,
        pending: None,
        frontend_starting: false,
        applications: Applications::discover(),
        active_entries: SearchIndex::new(Vec::new()),
        native_mode: false,
        calculator,
        saved: config::load_state(),
        ui_config: config::load_config(),
    }));

    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, state) {
                        eprintln!("connection error: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }

    Ok(())
}
