use std::{
    env,
    io::Read,
    process::{Command, Stdio},
    sync::{Arc, Condvar, Mutex, mpsc::Sender},
    thread,
    time::Duration,
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    config::config_home,
    daemon::broker::BrokerMessage,
    providers::api::{ActionResult, ProviderTrait},
};

/// One collector invocation's timeout — the collectors walk session files on
/// a cold cache, so this needs to be generous.
const COLLECT_TIMEOUT: Duration = Duration::from_secs(90);
const DEFAULT_POLL_SECS: u64 = 300;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AiUsageConfig {
    pub poll_secs: Option<u64>,
    pub codex_bin: Option<String>,
    pub opencode_bin: Option<String>,
}

impl AiUsageConfig {
    pub fn load() -> Self {
        #[derive(Deserialize, Default)]
        struct File {
            ai: Option<AiUsageConfig>,
        }

        let text =
            std::fs::read_to_string(config_home().join("yobu/config.toml")).unwrap_or_default();
        toml::from_str::<File>(&text)
            .ok()
            .and_then(|f| f.ai)
            .unwrap_or_default()
    }

    fn poll_secs(&self) -> Duration {
        Duration::from_secs(self.poll_secs.unwrap_or(DEFAULT_POLL_SECS).max(30))
    }
}

struct Source {
    topic: &'static str,
    bin: String,
}

impl Source {
    fn new(topic: &'static str, configured: Option<&str>, collector: &str) -> Self {
        let bin = configured.map(Into::into).unwrap_or_else(|| {
            let home = env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default();
            home.join(".config/quickshell/bin")
                .join(collector)
                .display()
                .to_string()
        });
        Self { topic, bin }
    }

    /// Run the collector and parse its single JSON record.
    ///
    /// yobud runs a global SIGCHLD reaper that reaps every child, so the
    /// exit status is usually already gone — completion is detected via
    /// stdout EOF instead, and ECHILD is treated as "exited and reaped".
    fn collect(&self) -> Result<Value, anyhow::Error> {
        let mut child = Command::new(&self.bin)
            .arg("--limits-only")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("spawn {}: {e}", self.bin))?;

        let mut stdout = child.stdout.take().expect("stdout piped");
        let reader = thread::spawn(move || {
            let mut buf = String::new();
            let _ = stdout.read_to_string(&mut buf);
            buf
        });

        let deadline = std::time::Instant::now() + COLLECT_TIMEOUT;
        let output = loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow::anyhow!("{} timed out", self.bin));
            }
            if reader.is_finished() {
                break reader
                    .join()
                    .map_err(|_| anyhow::anyhow!("collector reader panicked"))?;
            }
            thread::sleep(Duration::from_millis(50));
        };

        // stdout hit EOF, so the collector has exited — the global reaper
        // may have taken the status already (ECHILD), which is fine.
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                return Err(anyhow::anyhow!("{} exited with {status}", self.bin));
            }
            Ok(_) => {}
            Err(e) if e.raw_os_error() == Some(libc::ECHILD) => {}
            Err(e) => return Err(anyhow::anyhow!("wait {}: {e}", self.bin)),
        }
        let record: Value = serde_json::from_str(output.trim())
            .map_err(|e| anyhow::anyhow!("parse {}: {e}", self.bin))?;
        if record.get("id").is_none() {
            return Err(anyhow::anyhow!("{} produced no record id", self.bin));
        }
        Ok(record)
    }

    fn availability(&self, error: &str) -> Value {
        json!({ "id": self.topic.rsplit('.').next().unwrap_or(self.topic), "available": false, "error": error })
    }
}

struct Shared {
    /// Last published payload per topic, for the `status` action.
    records: Mutex<Vec<(String, Value)>>,
    /// Set by `refresh` so the polling loop wakes up early.
    wake: (Mutex<bool>, Condvar),
}

pub struct AiUsageProvider {
    sources: Vec<Source>,
    shared: Arc<Shared>,
    poll_secs: Duration,
}

impl AiUsageProvider {
    pub fn new(config: AiUsageConfig) -> Self {
        Self {
            sources: vec![
                Source::new("ai.codex", config.codex_bin.as_deref(), "agent-usage-codex"),
                Source::new(
                    "ai.opencode",
                    config.opencode_bin.as_deref(),
                    "agent-usage-opencode",
                ),
            ],
            shared: Arc::new(Shared {
                records: Mutex::new(Vec::new()),
                wake: (Mutex::new(false), Condvar::new()),
            }),
            poll_secs: config.poll_secs(),
        }
    }

    fn record_for(&self, topic: &str) -> Option<Value> {
        self.shared
            .records
            .lock()
            .unwrap()
            .iter()
            .find(|(t, _)| t == topic)
            .map(|(_, v)| v.clone())
    }
}

impl Default for AiUsageProvider {
    fn default() -> Self {
        Self::new(AiUsageConfig::load())
    }
}

impl ProviderTrait for AiUsageProvider {
    fn name(&self) -> &str {
        "ai"
    }

    fn init(&mut self, tx: Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
        let shared = self.shared.clone();
        let sources = self.sources.clone();
        let poll_secs = self.poll_secs;
        let tx = tx.clone();

        thread::spawn(move || {
            loop {
                for source in &sources {
                    let payload = match source.collect() {
                        Ok(record) => record,
                        Err(e) => {
                            eprintln!("ai-usage {}: {e}", source.topic);
                            source.availability(&e.to_string())
                        }
                    };

                    let mut records = shared.records.lock().unwrap();
                    if let Some(slot) = records.iter_mut().find(|(t, _)| t == source.topic) {
                        slot.1 = payload.clone();
                    } else {
                        records.push((source.topic.to_string(), payload.clone()));
                    }
                    drop(records);

                    tx.send(BrokerMessage::Publish {
                        topic: source.topic.to_string(),
                        payload,
                    })
                    .expect("failed to publish ai usage");
                }

                // Sleep for the poll interval, or wake early when `refresh` fires.
                let (flag, cvar) = &shared.wake;
                let mut wake = flag.lock().unwrap();
                let deadline = std::time::Instant::now() + poll_secs;
                loop {
                    // Check before waiting: the flag may have been set during
                    // the poll above, after the (missed) condvar notification.
                    if *wake {
                        *wake = false;
                        break;
                    }
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    let (woken, timeout) = cvar.wait_timeout(wake, remaining).unwrap();
                    wake = woken;
                    if *wake {
                        *wake = false;
                        break;
                    }
                    if timeout.timed_out() && std::time::Instant::now() >= deadline {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    fn get_actions(&self) -> Vec<String> {
        vec!["status".into(), "refresh".into()]
    }

    fn run_action(
        &mut self,
        action: &str,
        _: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            "status" => Ok(ActionResult {
                result: json!({
                    "codex": self.record_for("ai.codex"),
                    "opencode": self.record_for("ai.opencode"),
                }),
                events: vec![],
            }),
            "refresh" => {
                let (flag, cvar) = &self.shared.wake;
                *flag.lock().unwrap() = true;
                cvar.notify_all();
                Ok(ActionResult {
                    result: json!({ "queued": true }),
                    events: vec![],
                })
            }
            _ => Err(anyhow::anyhow!("Unknown action: {action}")),
        }
    }
}

impl Clone for Source {
    fn clone(&self) -> Self {
        Self {
            topic: self.topic,
            bin: self.bin.clone(),
        }
    }
}
