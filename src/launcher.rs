//! Delegate application ownership to the systemd user manager.
use serde::Deserialize;
use std::{
    ffi::OsString,
    io,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Backend {
    #[default]
    Auto,
    Uwsm,
    Systemd,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchConfig {
    #[serde(default)]
    backend: Backend,
}

#[derive(Default, Deserialize)]
struct Config {
    #[serde(default)]
    launch: LaunchConfig,
}

fn config() -> io::Result<LaunchConfig> {
    match std::fs::read_to_string(crate::config::config_home().join("yobu/config.toml")) {
        Ok(text) => toml::from_str::<Config>(&text)
            .map(|c| c.launch)
            .map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(LaunchConfig::default()),
        Err(e) => Err(e),
    }
}

// Poll and reap helpers: never leave launcher children as zombies or wait indefinitely.
fn run(command: &mut Command) -> io::Result<()> {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::null()).spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "launch helper exited with {status}"
                )))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "launch helper timed out; application may have started, not retrying",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn select(requested: Backend, uwsm_active: bool, systemd_active: bool) -> io::Result<Backend> {
    match requested {
        Backend::Auto if uwsm_active => Ok(Backend::Uwsm),
        Backend::Auto | Backend::Systemd if systemd_active => Ok(Backend::Systemd),
        Backend::Uwsm if uwsm_active => Ok(Backend::Uwsm),
        _ => Err(io::Error::other(
            "requested launch backend unavailable: requires an active UWSM session or systemd user manager",
        )),
    }
}

fn args(
    backend: Backend,
    desktop: &Path,
    command: &[String],
    terminal: bool,
) -> io::Result<Vec<OsString>> {
    if backend == Backend::Uwsm {
        return Ok(vec![
            "app".into(),
            "-t".into(),
            "service".into(),
            "--".into(),
            desktop.as_os_str().to_owned(),
        ]);
    }
    if terminal {
        return Err(io::Error::other(
            "terminal desktop entries require the uwsm launch backend",
        ));
    }
    if command.is_empty() {
        return Err(io::Error::other("empty application command"));
    }
    let mut args: Vec<OsString> = [
        "--user",
        "--collect",
        "--service-type=exec",
        "--expand-environment=no",
        "--slice=app.slice",
        "--quiet",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    // Retain the user manager's environment; override only session-specific values
    // present in yobud, without copying secrets or the entire launcher environment.
    for key in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_TYPE",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "NIRI_SOCKET",
        "HYPRLAND_INSTANCE_SIGNATURE",
    ] {
        if let Some(value) = std::env::var_os(key) {
            let mut arg = OsString::from(format!("--setenv={key}="));
            arg.push(value);
            args.push(arg);
        }
    }
    args.push("--".into());
    args.extend(command.iter().map(OsString::from));
    Ok(args)
}

pub fn launch(desktop: &Path, command: &[String], terminal: bool) -> io::Result<()> {
    let requested = config()?.backend;
    let uwsm_active = requested != Backend::Systemd
        && run(Command::new("uwsm")
            .args(["check", "is-active"])
            .stderr(Stdio::null()))
        .is_ok();
    let systemd_active = !uwsm_active
        && requested != Backend::Uwsm
        && run(Command::new("systemctl")
            .args(["--user", "show-environment"])
            .stderr(Stdio::null()))
        .is_ok();
    let backend = select(requested, uwsm_active, systemd_active)?;
    let program = if backend == Backend::Uwsm {
        "uwsm"
    } else {
        "systemd-run"
    };
    // A failed launch is returned to the caller, never retried with another backend.
    run(Command::new(program).args(args(backend, desktop, command, terminal)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_exit_status_is_checked() {
        assert!(run(Command::new("/bin/sh").args(["-c", "exit 0"])).is_ok());
        assert!(run(Command::new("/bin/sh").args(["-c", "exit 7"])).is_err());
        assert!(run(&mut Command::new("/nonexistent/yobu-launch-helper")).is_err());
    }

    #[test]
    fn selection_requires_runtime_readiness() {
        assert_eq!(select(Backend::Auto, true, true).unwrap(), Backend::Uwsm);
        assert_eq!(
            select(Backend::Auto, false, true).unwrap(),
            Backend::Systemd
        );
        assert!(select(Backend::Auto, false, false).is_err());
        assert!(select(Backend::Uwsm, false, true).is_err());
        assert!(select(Backend::Systemd, true, false).is_err());
    }

    #[test]
    fn configuration_is_explicit_and_strict() {
        assert_eq!(
            toml::from_str::<Config>("").unwrap().launch.backend,
            Backend::Auto
        );
        assert_eq!(
            toml::from_str::<Config>("[launch]\nbackend='systemd'")
                .unwrap()
                .launch
                .backend,
            Backend::Systemd
        );
        assert!(toml::from_str::<Config>("[launch]\nbackend='direct'").is_err());
        assert!(toml::from_str::<Config>("[launch]\nbacked='uwsm'").is_err());
    }

    #[test]
    fn uwsm_receives_desktop_path_not_shell_command() {
        let actual = args(
            Backend::Uwsm,
            Path::new("/tmp/app space.desktop"),
            &[],
            true,
        )
        .unwrap();
        assert_eq!(
            actual,
            ["app", "-t", "service", "--", "/tmp/app space.desktop"].map(OsString::from)
        );
    }

    #[test]
    fn systemd_preserves_literal_arguments_and_rejects_terminal_entries() {
        let command = vec![
            "example".into(),
            "$HOME space; echo nope".into(),
            "--flag".into(),
        ];
        let actual = args(Backend::Systemd, Path::new("unused"), &command, false).unwrap();
        assert!(actual.contains(&OsString::from("--service-type=exec")));
        assert!(actual.contains(&OsString::from("--expand-environment=no")));
        assert_eq!(
            &actual[actual.len() - 4..],
            &[
                OsString::from("--"),
                OsString::from("example"),
                OsString::from("$HOME space; echo nope"),
                OsString::from("--flag")
            ]
        );
        assert!(args(Backend::Systemd, Path::new("unused"), &command, true).is_err());
    }
}
