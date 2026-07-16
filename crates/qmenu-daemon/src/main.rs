mod apps;
mod config;

use apps::Applications;
use config::SavedState;
use qmenu_protocol::{Message, Role};
use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

struct State {
    frontend: Option<UnixStream>,
    client: Option<UnixStream>,
    pending: Option<Message>,
    frontend_starting: bool,
    applications: Applications,
    saved: SavedState,
    ui_config: qmenu_protocol::UiConfig,
}

fn socket_path() -> PathBuf {
    env::var_os("QMENU_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
                .join("qmenu.sock")
        })
}

fn send(stream: &mut UnixStream, message: &Message) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn spawn_frontend() -> io::Result<()> {
    let frontend = env::var_os("QMENU_FRONTEND")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("frontend/shell.qml"));
    Command::new("qs").arg("-p").arg(frontend).spawn()?;
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
                    if let Err(error) = shared.applications.launch(value)
                        && let Some(client) = shared.client.as_mut()
                    {
                        send(
                            client,
                            &Message::Error {
                                message: error.to_string(),
                            },
                        )?;
                    }
                }
                if let Some(mut client) = shared.client.take() {
                    send(&mut client, &message)?;
                }
            }
            Message::Cancelled => {
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
                    .insert(screen, qmenu_protocol::Position { x, y });
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
        prompt, entries, ..
    } = request
    else {
        return Ok(());
    };

    let (message, should_spawn) = {
        let mut shared = state.lock().unwrap();
        let entries = if entries.is_empty() {
            shared.applications.entries.clone()
        } else {
            entries
        };
        let message = Message::Show {
            prompt,
            entries,
            positions: shared.saved.positions.clone(),
            config: shared.ui_config.clone(),
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

fn main() -> io::Result<()> {
    let path = socket_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    println!("qmenu daemon listening on {}", path.display());

    let state = Arc::new(Mutex::new(State {
        frontend: None,
        client: None,
        pending: None,
        frontend_starting: false,
        applications: Applications::discover(),
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
