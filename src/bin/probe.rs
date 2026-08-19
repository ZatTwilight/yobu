use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

use serde_json::json;
use yobu::protocol::Envelope;

fn socket_path() -> PathBuf {
    env::var_os("YOBU_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
                .join("yobu.sock")
        })
}

fn send(stream: &mut UnixStream, message: &Envelope) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn main() -> io::Result<()> {
    let mut stream = UnixStream::connect(socket_path())?;
    send(
        &mut stream,
        &Envelope::Hello {
            name: "probe".into(),
            capabilities: vec!["applications.consumer".into()],
        },
    )?;
    send(
        &mut stream,
        &Envelope::Subscribe {
            topic: "applications.changed".into(),
        },
    )?;
    send(
        &mut stream,
        &Envelope::Request {
            id: 1,
            method: "cpu.current".into(),
            params: json!({}),
        },
    )?;
    // send(
    //     &mut stream,
    //     &Envelope::Request {
    //         id: 1,
    //         method: "applications.list".into(),
    //         params: json!({}),
    //     },
    // )?;
    // send(
    //     &mut stream,
    //     &Envelope::Request {
    //         id: 2,
    //         method: "applications.refresh".into(),
    //         params: json!({}),
    //     },
    // )?;

    for line in BufReader::new(stream).lines().take(3) {
        println!("{}", line?);
    }

    Ok(())
}
