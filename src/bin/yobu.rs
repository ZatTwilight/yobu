use std::{
    env,
    io::{self, BufRead, BufReader, IsTerminal, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use yobu::protocol::{Entry, Message, Role};

const DEFAULT_VIEW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/frontend/LauncherView.qml"
));

fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".config")
        })
}

fn init_view() -> io::Result<()> {
    let path = config_home().join("yobu/LauncherView.qml");
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} already exists; remove it before initializing",
                path.display()
            ),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, DEFAULT_VIEW)?;
    println!("Created {}", path.display());
    Ok(())
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

fn connect() -> io::Result<UnixStream> {
    UnixStream::connect(socket_path()).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot connect to yobu daemon: {error}"),
        )
    })
}

fn show(
    stream: &mut UnixStream,
    prompt: String,
    entries: Vec<Entry>,
    benchmark: bool,
) -> io::Result<()> {
    send(stream, &Message::Hello { role: Role::Client })?;
    send(
        stream,
        &Message::Show {
            prompt,
            entries,
            positions: Default::default(),
            config: Default::default(),
            benchmark,
        },
    )
}

fn read_entries() -> io::Result<Vec<Entry>> {
    if io::stdin().is_terminal() {
        return Ok(Vec::new());
    }

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
        .collect()
}

fn run_menu(prompt: String) -> io::Result<()> {
    let entries = read_entries()?;
    let mut stream = connect()?;
    show(&mut stream, prompt, entries, false)?;

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

fn measure_display() -> io::Result<Duration> {
    let started = Instant::now();
    let mut stream = connect()?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    show(&mut stream, "Benchmark".into(), Vec::new(), true)?;

    if let Some(line) = BufReader::new(stream).lines().next() {
        match serde_json::from_str::<Message>(&line?)? {
            Message::Presented => return Ok(started.elapsed()),
            Message::Error { message } => return Err(io::Error::other(message)),
            message => {
                return Err(io::Error::other(format!(
                    "unexpected benchmark response: {message:?}"
                )));
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "frontend disconnected before presenting the launcher",
    ))
}

fn print_summary(name: &str, samples: &[Duration]) {
    if samples.is_empty() {
        return;
    }

    let mut millis: Vec<f64> = samples
        .iter()
        .map(|duration| duration.as_secs_f64() * 1_000.0)
        .collect();
    millis.sort_by(f64::total_cmp);
    let mean = millis.iter().sum::<f64>() / millis.len() as f64;
    let percentile =
        |p: f64| millis[((millis.len() as f64 * p).ceil() as usize - 1).min(millis.len() - 1)];

    println!(
        "{name}: n={}, min={:.3} ms, mean={mean:.3} ms, p50={:.3} ms, p95={:.3} ms, max={:.3} ms",
        millis.len(),
        millis[0],
        percentile(0.50),
        percentile(0.95),
        millis[millis.len() - 1],
    );
}

fn benchmark_display(iterations: usize) -> io::Result<()> {
    if iterations == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "benchmark iteration count must be greater than zero",
        ));
    }

    let mut samples = Vec::with_capacity(iterations);
    for iteration in 1..=iterations {
        let elapsed = measure_display()?;
        println!(
            "display[{iteration}]: {:.3} ms",
            elapsed.as_secs_f64() * 1_000.0
        );
        samples.push(elapsed);

        // Let the compositor process the unmap before requesting another map.
        // The delay is outside the measured interval.
        if iteration < iterations {
            thread::sleep(Duration::from_millis(100));
        }
    }

    print_summary("all displays", &samples);
    if samples.len() > 1 {
        print_summary("warm displays (excluding first)", &samples[1..]);
    }
    if samples.len() > 2 {
        print_summary("steady displays (excluding first two)", &samples[2..]);
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut args = env::args().skip(1);
    match args.next() {
        Some(flag) if flag == "--init-view" => init_view(),
        Some(flag) if flag == "--benchmark-display" => {
            let iterations = args
                .next()
                .map(|value| {
                    value.parse::<usize>().map_err(|error| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("invalid benchmark iteration count: {error}"),
                        )
                    })
                })
                .transpose()?
                .unwrap_or(10);
            benchmark_display(iterations)
        }
        prompt => run_menu(prompt.unwrap_or_else(|| "Applications".into())),
    }
}
