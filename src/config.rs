use crate::protocol::{Position, UiConfig};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrecencyEntry {
    #[serde(default = "default_count")]
    pub count: u32,
    pub last_access: u64,
}

fn default_count() -> u32 {
    1
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SavedState {
    #[serde(default)]
    pub positions: HashMap<String, Position>,
    /// Legacy MRU list — kept for migration, new code uses `frecency`.
    #[serde(default)]
    pub recent_applications: Vec<String>,
    #[serde(default)]
    pub frecency: HashMap<String, FrecencyEntry>,
}

impl SavedState {
    pub fn record_application(&mut self, id: &str) {
        let now = now_unix();
        let entry = self.frecency.entry(id.to_owned()).or_insert(FrecencyEntry {
            count: 0,
            last_access: now,
        });
        entry.count = entry.count.saturating_add(1);
        entry.last_access = now;

        // Keep legacy vec in sync for downgrade/back-compat.
        self.recent_applications.retain(|recent| recent != id);
        self.recent_applications.insert(0, id.to_owned());
        self.recent_applications.truncate(100);
    }

    pub fn frecency_score(&self, id: &str, now: u64) -> f64 {
        let Some(entry) = self.frecency.get(id) else {
            return 0.0;
        };
        let delta = now.saturating_sub(entry.last_access) as f64;
        // Half-life 14 days — mirrors fff's frecency decay idea.
        let half_life = 14.0 * 24.0 * 3600.0;
        let recency_factor = 0.5_f64.powf(delta / half_life);
        entry.count as f64 * recency_factor
    }

    fn migrate(&mut self) {
        if !self.frecency.is_empty() || self.recent_applications.is_empty() {
            return;
        }
        // Synthesize frecency from legacy MRU order: most recent = now, each older step = -1 day.
        let now = now_unix();
        for (rank, id) in self.recent_applications.iter().enumerate() {
            let last_access = now.saturating_sub((rank as u64) * 86400);
            self.frecency.insert(
                id.clone(),
                FrecencyEntry {
                    count: 1,
                    last_access,
                },
            );
        }
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_config() -> UiConfig {
    let path = config_home().join("yobu/config.toml");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn load_state() -> SavedState {
    let mut state: SavedState = fs::read_to_string(state_path())
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default();
    state.migrate();
    state
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
