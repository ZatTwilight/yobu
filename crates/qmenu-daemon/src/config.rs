use qmenu_protocol::{Position, UiConfig};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, io, path::PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SavedState {
    #[serde(default)]
    pub positions: HashMap<String, Position>,
}

pub fn load_config() -> UiConfig {
    let path = config_home().join("qmenu/config.toml");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn load_state() -> SavedState {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_state(state: &SavedState) -> io::Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let encoded = toml::to_string_pretty(state).map_err(io::Error::other)?;
    fs::write(path, encoded)
}

fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

fn state_path() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("qmenu/state.toml")
}

fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
