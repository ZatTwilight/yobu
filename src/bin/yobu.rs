use std::{
    env,
    io::{self, BufRead, BufReader, IsTerminal, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use yobu::protocol::{Entry, Envelope};

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

fn find_shell_path() -> Option<PathBuf> {
    if let Some(p) = env::var_os("YOBU_SHELL").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = env::var_os("YOBU_FRONTEND").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    for base in [
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| config_home()),
        PathBuf::from(env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".config"),
    ] {
        let p = base.join("quickshell/yobu/shell.qml");
        if p.is_file() {
            return Some(p);
        }
    }
    // Dev fallback: repo frontend relative to manifest
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend/shell.qml");
    if dev.is_file() {
        return Some(dev);
    }
    // Also try current dir
    let cwd = PathBuf::from("frontend/shell.qml");
    if cwd.is_file() {
        return Some(cwd);
    }
    None
}

fn qs_ipc_args(extra: &[String]) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(shell) = find_shell_path() {
        args.push("-p".to_owned());
        args.push(shell.display().to_string());
    }
    args.extend_from_slice(extra);
    args
}

fn run_qs(args: &[String]) -> io::Result<std::process::Output> {
    let all = qs_ipc_args(args);
    let output = Command::new("qs").arg("ipc").args(&all).output()?;
    Ok(output)
}

fn run_qs_success(args: &[String]) -> io::Result<()> {
    let out = run_qs(args)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_owned()
        } else {
            stdout.trim().to_owned()
        };
        return Err(io::Error::other(if msg.is_empty() {
            format!("qs ipc failed: {:?}", args)
        } else {
            msg
        }));
    }
    Ok(())
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

fn read_piped_entries() -> io::Result<Option<Vec<Entry>>> {
    if io::stdin().is_terminal() {
        return Ok(None);
    }
    let entries = io::stdin()
        .lock()
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let value = line?;
            // Tab-delimited producers (such as cliphist) keep their complete
            // value for stdout while presenting only the human-readable part.
            let label = value
                .split_once('\t')
                .map_or_else(|| value.clone(), |(_, label)| label.to_owned());
            Ok(Entry {
                id: i.to_string(),
                label,
                description: None,
                icon: None,
                preview: None,
                value,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok((!entries.is_empty()).then_some(entries))
}

fn read_json_entries() -> io::Result<Vec<Entry>> {
    io::stdin()
        .lock()
        .lines()
        .map(|line| serde_json::from_str::<Entry>(&line?).map_err(io::Error::other))
        .collect()
}

fn cliphist_entries(limit: usize) -> io::Result<Vec<Entry>> {
    let output = Command::new("cliphist").arg("list").output()?;
    if !output.status.success() {
        return Err(io::Error::other("cliphist list failed"));
    }
    let preview_dir =
        PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
            .join("yobu/cliphist-previews");
    std::fs::create_dir_all(&preview_dir)?;

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(limit)
        .enumerate()
        .map(|(index, value)| {
            let (_, label) = value.split_once('\t').unwrap_or(("", value));
            let preview = if label.starts_with("[[ binary data") {
                let path = preview_dir.join(format!(
                    "{}.image",
                    value.split('\t').next().unwrap_or("unknown")
                ));
                if path.exists() {
                    Some(format!("file://{}", path.display()))
                } else {
                    let mut child = Command::new("cliphist")
                        .arg("decode")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .spawn()?;
                    child.stdin.as_mut().unwrap().write_all(value.as_bytes())?;
                    let decoded = child.wait_with_output()?;
                    if decoded.status.success() {
                        std::fs::write(&path, decoded.stdout)?;
                        Some(format!("file://{}", path.display()))
                    } else {
                        None
                    }
                }
            } else {
                None
            };
            Ok(Entry {
                id: index.to_string(),
                label: label.to_owned(),
                description: None,
                icon: None,
                preview,
                value: value.to_owned(),
            })
        })
        .collect()
}

fn socket_path() -> PathBuf {
    env::var_os("YOBU_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()))
                .join("yobu.sock")
        })
}

struct DaemonClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
}

impl DaemonClient {
    fn connect() -> io::Result<Self> {
        let mut stream = UnixStream::connect(socket_path())
            .map_err(|e| io::Error::new(e.kind(), format!("cannot connect to yobud: {e}")))?;
        let reader = BufReader::new(stream.try_clone()?);
        serde_json::to_writer(
            &mut stream,
            &Envelope::Hello {
                name: format!("yobu.cli.{}", std::process::id()),
                capabilities: vec!["menus.client".into()],
            },
        )?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        Ok(Self {
            stream,
            reader,
            next_id: 1,
        })
    }

    fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> io::Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;
        serde_json::to_writer(
            &mut self.stream,
            &Envelope::Request {
                id,
                method: method.into(),
                params,
            },
        )?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()?;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            match serde_json::from_str::<Envelope>(&line)? {
                Envelope::Response {
                    id: response_id,
                    result,
                } if response_id == id => return Ok(result),
                Envelope::Error {
                    id: response_id,
                    message,
                    ..
                } if response_id == id => return Err(io::Error::other(message)),
                _ => {}
            }
        }
    }
}

fn do_toggle() -> io::Result<()> {
    run_qs_success(&["call".into(), "yobu".into(), "toggle".into()])
}

fn do_show() -> io::Result<()> {
    run_qs_success(&[
        "call".into(),
        "yobu".into(),
        "setVisible".into(),
        "true".into(),
    ])
}

fn do_hide() -> io::Result<()> {
    run_qs_success(&[
        "call".into(),
        "yobu".into(),
        "setVisible".into(),
        "false".into(),
    ])
}

fn do_piped(prompt: String, entries: Vec<Entry>) -> io::Result<()> {
    let mut daemon = DaemonClient::connect()?;
    let created = daemon.request(
        "menus.create",
        serde_json::json!({ "prompt": prompt, "entries": entries }),
    )?;
    let key = created
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| io::Error::other("yobud returned no menu key"))?
        .to_owned();
    run_qs_success(&["call".into(), "yobu".into(), "showMenu".into(), key.clone()])?;

    loop {
        thread::sleep(Duration::from_millis(50));
        let result = daemon.request("menus.take_result", serde_json::json!({ "key": key }))?;
        if !result
            .get("done")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        match result.get("value") {
            Some(serde_json::Value::String(value)) => {
                println!("{value}");
                return Ok(());
            }
            _ => std::process::exit(1),
        }
    }
}

fn print_help() {
    println!(
        "yobu — tiny wrapper for Quickshell IPC\n\
        \n\
        Usage:\n  \
          yobu [prompt]              toggle launcher (or piped if stdin)\n  \
          yobu --show                show launcher\n  \
          yobu --hide                hide launcher\n  \
          yobu --toggle              toggle launcher\n  \
          printf 'a\\nb\\nc' | yobu \"Pick one\"   piped menu, prints selection\n  \
          yobu --init-view           create ~/.config/yobu/LauncherView.qml\n\
        \n\
        Env:\n  \
          YOBU_SHELL   path to shell.qml for qs -p"
    );
}

fn main() -> io::Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();

    if args.first().map(|s| s.as_str()) == Some("--help")
        || args.first().map(|s| s.as_str()) == Some("-h")
    {
        print_help();
        return Ok(());
    }
    if args.first().map(|s| s.as_str()) == Some("--init-view") {
        return init_view();
    }
    if args.first().map(|s| s.as_str()) == Some("--cliphist") {
        return do_piped("Clipboard".into(), cliphist_entries(500)?);
    }
    if args.first().map(|s| s.as_str()) == Some("--json-lines") {
        args.remove(0);
        let prompt = args.into_iter().next().unwrap_or_else(|| "Pick one".into());
        return do_piped(prompt, read_json_entries()?);
    }

    // explicit show/hide/toggle flags
    if let Some(flag) = args.first().cloned() {
        match flag.as_str() {
            "--show" => return do_show(),
            "--hide" => return do_hide(),
            "--toggle" => return do_toggle(),
            "--" => {
                args.remove(0);
            }
            _ if flag.starts_with('-') => {
                eprintln!("unknown flag: {flag}");
                print_help();
                std::process::exit(2);
            }
            _ => {}
        }
    }

    // piped?
    if let Some(entries) = read_piped_entries()? {
        let prompt = args.into_iter().next().unwrap_or_else(|| "Pick one".into());
        return do_piped(prompt, entries);
    }

    // no pipe: if prompt given as single arg without pipe, treat as show with toggle? For now, toggle.
    // If user passed a prompt without pipe, just show launcher (ignore prompt for normal mode)
    if !args.is_empty() {
        // Could be `yobu "Applications"` — just toggle/show
        return do_toggle();
    }

    do_toggle()
}
