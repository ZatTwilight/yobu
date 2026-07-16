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

#[derive(Default)]
struct State {
    frontend: Option<UnixStream>,
    client: Option<UnixStream>,
    pending: Option<Message>,
    frontend_starting: bool,
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
        } => {
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
                if matches!(message, Message::Selected { .. } | Message::Cancelled) {
                    let mut shared = state.lock().unwrap();
                    if let Some(mut client) = shared.client.take() {
                        send(&mut client, &message)?;
                    }
                }
            }

            let mut shared = state.lock().unwrap();
            shared.frontend = None;
            shared.frontend_starting = false;
        }
        Message::Hello { role: Role::Client } => {
            let Some(line) = lines.next() else {
                return Ok(());
            };
            let request: Message = serde_json::from_str(&line?)?;
            if !matches!(request, Message::Show { .. }) {
                return Ok(());
            }

            let should_spawn = {
                let mut shared = state.lock().unwrap();
                shared.client = Some(stream.try_clone()?);
                if let Some(frontend) = shared.frontend.as_mut() {
                    send(frontend, &request)?;
                    false
                } else {
                    shared.pending = Some(request);
                    if shared.frontend_starting {
                        false
                    } else {
                        shared.frontend_starting = true;
                        true
                    }
                }
            };

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
        }
        _ => {}
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

    let state = Arc::new(Mutex::new(State::default()));
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
