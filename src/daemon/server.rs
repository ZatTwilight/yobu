use std::{
    collections::HashSet,
    env, fs,
    io::{self, BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    thread,
};

use crate::{
    daemon::{
        broker::{Broker, BrokerMessage},
        client::{Client, ClientId},
    },
    protocol::Envelope,
    providers::{applications::ApplicationsProvider, cpu::CpuProvider},
    util::{IdGen, RunOnExit},
};

fn socket_path() -> PathBuf {
    env::var_os("YOBU_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
                .join("yobu.sock")
        })
}

fn write_to_stream(stream: &mut UnixStream, message: &Envelope) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn spawn_writer(
    mut stream: UnixStream,
    rx: Receiver<Envelope>,
    broker_tx: Sender<BrokerMessage>,
    id: ClientId,
) {
    thread::spawn(move || {
        for msg in rx {
            if let Err(e) = write_to_stream(&mut stream, &msg) {
                eprintln!("failed to write to client {}: {}", id.0, e);
                if let Err(e) = broker_tx.send(BrokerMessage::WriterFailed {
                    id,
                    error: e.to_string(),
                }) {
                    eprintln!("failed to send disconnect message: {e}");
                }
                if let Err(e) = stream.shutdown(Shutdown::Both) {
                    eprintln!("failed to shutdown stream: {e}");
                }
                break;
            }
        }
    });
}

pub fn handle_connection(
    stream: UnixStream,
    id: u64,
    broker_tx: Sender<BrokerMessage>,
) -> io::Result<()> {
    let reader_stream = stream.try_clone()?;
    let mut lines = BufReader::new(reader_stream).lines();
    // Read first line/message (Should be a hello)
    let Some(line) = lines.next() else {
        return Ok(());
    };
    let msg: Envelope = serde_json::from_str(&line?)?;

    let c = match msg {
        Envelope::Hello { name, capabilities } => {
            // Register client
            let (tx, rx) = std::sync::mpsc::channel();
            let client_id = ClientId(id);
            spawn_writer(stream, rx, broker_tx.clone(), client_id);
            Client {
                id: client_id,
                name,
                capabilities: capabilities.into_iter().collect(),
                outbound: tx,
                subscriptions: HashSet::new(),
            }
        }
        _ => return Ok(()),
    };
    let id = c.id;
    broker_tx
        .send(BrokerMessage::Connect(c))
        .map_err(io::Error::other)?;

    let guard_tx = broker_tx.clone();
    let _guard = RunOnExit(Some(move || {
        if let Err(e) = guard_tx.send(BrokerMessage::Disconnect(id)) {
            eprintln!("failed to send disconnect message: {e}");
        }
    }));

    // Message from client
    for line in lines {
        let line = line?;
        if let Ok(msg) = serde_json::from_str::<Envelope>(&line)
            && let Err(e) = broker_tx.send(BrokerMessage::Inbound {
                from: id,
                message: msg,
            })
        {
            eprintln!("failed to send client message: {e}");
            break;
        }
    }

    Ok(())
}

pub fn start() -> io::Result<()> {
    let path = socket_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    println!("yobu daemon listening on {}", path.display());

    let (mut broker, broker_tx) = Broker::new();
    broker
        .register_provider(Box::new(ApplicationsProvider::new()))
        .map_err(io::Error::other)?;
    broker
        .register_provider(Box::new(CpuProvider::new()))
        .map_err(io::Error::other)?;
    broker
        .init_providers(broker_tx.clone())
        .map_err(io::Error::other)?;
    thread::spawn(move || broker.listen());

    let id_gen = IdGen::new();

    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let id = id_gen.next();
                let tx = broker_tx.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, id, tx) {
                        eprintln!("connection error: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }

    Ok(())
}
