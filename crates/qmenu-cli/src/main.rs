use qmenu_protocol::{Entry, Message, Role};
use std::{
    env,
    io::{self, BufRead, BufReader, IsTerminal, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

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

fn main() -> io::Result<()> {
    let prompt = env::args().nth(1).unwrap_or_else(|| "Applications".into());
    let entries = if io::stdin().is_terminal() {
        Vec::new()
    } else {
        io::stdin()
            .lock()
            .lines()
            .enumerate()
            .map(|(index, line)| {
                let value = line?;
                Ok(Entry {
                    id: index.to_string(),
                    label: value.clone(),
                    description: None,
                    icon: None,
                    value,
                })
            })
            .collect::<io::Result<Vec<_>>>()?
    };

    let mut stream = UnixStream::connect(socket_path()).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot connect to qmenu daemon: {error}"),
        )
    })?;
    send(&mut stream, &Message::Hello { role: Role::Client })?;
    send(
        &mut stream,
        &Message::Show {
            prompt,
            entries,
            positions: Default::default(),
            config: Default::default(),
        },
    )?;

    let mut lines = BufReader::new(stream).lines();
    if let Some(line) = lines.next() {
        match serde_json::from_str::<Message>(&line?)? {
            Message::Selected { value } if !value.starts_with("app:") => println!("{value}"),
            Message::Selected { .. } => {}
            Message::Cancelled => std::process::exit(1),
            Message::Error { message } => return Err(io::Error::other(message)),
            _ => {}
        }
    }

    Ok(())
}
