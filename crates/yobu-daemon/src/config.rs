use yobu_protocol::{Position, UiConfig};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, io, path::PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SavedState {
    #[serde(default)]
    pub positions: HashMap<String, Position>,
    /// Application IDs in most-recently-used order.
    #[serde(default)]
    pub recent_applications: Vec<String>,
}

impl SavedState {
    pub fn record_application(&mut self, id: &str) {
        self.recent_applications.retain(|recent| recent != id);
        self.recent_applications.insert(0, id.to_owned());
        self.recent_applications.truncate(100);
    }
}

pub fn load_config() -> UiConfig {
    let path = config_home().join("yobu/config.toml");
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

pub fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

fn state_path() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("yobu/state.toml")
}

fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
