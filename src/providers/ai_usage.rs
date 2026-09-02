use std::{
    env,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    config::config_home,
    daemon::broker::BrokerMessage,
    providers::{
        ai_usage_native::{codex_record, opencode_record},
        api::{ActionResult, ProviderTrait},
    },
};

const CODEX_RPC_TIMEOUT: Duration = Duration::from_secs(8);
const DEFAULT_POLL_SECS: u64 = 300;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AiUsageConfig {
    pub poll_secs: Option<u64>,
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
            .and_then(|file| file.ai)
            .unwrap_or_default()
    }

    fn poll_secs(&self) -> Duration {
        Duration::from_secs(self.poll_secs.unwrap_or(DEFAULT_POLL_SECS).max(30))
    }
}

fn rpc_request(
    child: &mut Child,
    responses: &Receiver<Value>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, anyhow::Error> {
    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("codex app-server stdin closed"))?;
    serde_json::to_writer(
        &mut *stdin,
        &json!({ "id": id, "method": method, "params": params }),
    )?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;

    let deadline = std::time::Instant::now() + CODEX_RPC_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let message = responses
            .recv_timeout(remaining)
            .map_err(|error| anyhow::anyhow!("codex {method}: {error}"))?;
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(anyhow::anyhow!("codex {method}: {error}"));
        }
        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
}

fn codex_limits() -> Result<Value, anyhow::Error> {
    // This is the only external process used by the AI usage provider. Codex
    // exposes account rate limits through its official app-server protocol.
    let mut child = Command::new("codex")
        .args(["-s", "read-only", "-a", "never", "app-server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| anyhow::anyhow!("spawn codex app-server: {error}"))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let (response_tx, response_rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(message) = serde_json::from_str(&line) {
                let _ = response_tx.send(message);
            }
        }
    });

    let result = (|| {
        rpc_request(
            &mut child,
            &response_rx,
            1,
            "initialize",
            json!({ "clientInfo": { "name": "yobu", "version": env!("CARGO_PKG_VERSION") } }),
        )?;
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("codex app-server stdin closed"))?;
        serde_json::to_writer(
            &mut *stdin,
            &json!({ "method": "initialized", "params": {} }),
        )?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;

        let account = rpc_request(&mut child, &response_rx, 2, "account/read", json!({}))?;
        let response = rpc_request(
            &mut child,
            &response_rx,
            3,
            "account/rateLimits/read",
            json!({}),
        )?;
        let rate_limits = response.get("rateLimits").cloned().unwrap_or(Value::Null);
        let plan = rate_limits
            .get("planType")
            .or_else(|| account.pointer("/account/planType"))
            .cloned()
            .unwrap_or(Value::String(String::new()));
        let limits = ["primary", "secondary"]
            .into_iter()
            .filter_map(|name| {
                let window = rate_limits.get(name)?;
                let used = window.get("usedPercent")?.as_f64()?;
                let minutes = window
                    .get("windowDurationMins")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let label = match minutes {
                    10_080 => "Weekly (7-day)".to_string(),
                    value if value > 0 && value % 60 == 0 => format!("{}h window", value / 60),
                    value if value > 0 => format!("{value}m window"),
                    _ => "Limit".to_string(),
                };
                let resets_at = window
                    .get("resetsAt")
                    .and_then(Value::as_i64)
                    .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
                    .map(|time| time.to_rfc3339())
                    .unwrap_or_default();
                Some(json!({ "label": label, "percent": used / 100.0, "resetsAt": resets_at }))
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "limits": limits,
            "tierLabel": plan,
            "usageStatusText": "",
            "authHelpText": "Run `codex login` to authenticate."
        }))
    })();

    let _ = child.kill();
    let _ = child.wait();
    result
}

fn merge(record: &mut Value, fields: &Value) {
    if let (Some(record), Some(fields)) = (record.as_object_mut(), fields.as_object()) {
        record.extend(fields.clone());
    }
}

fn collect_codex() -> Value {
    let mut record = codex_record();
    match codex_limits() {
        Ok(limits) => merge(&mut record, &limits),
        Err(error) => merge(
            &mut record,
            &json!({
                "limits": [],
                "tierLabel": "",
                "usageStatusText": "Codex limits unavailable",
                "authHelpText": error.to_string(),
            }),
        ),
    }
    record
}

fn publish(shared: &Shared, tx: &Sender<BrokerMessage>, topic: &str, payload: Value) {
    let mut records = shared.records.lock().unwrap();
    if let Some(slot) = records.iter_mut().find(|(name, _)| name == topic) {
        slot.1 = payload.clone();
    } else {
        records.push((topic.to_string(), payload.clone()));
    }
    drop(records);
    tx.send(BrokerMessage::Publish {
        topic: topic.to_string(),
        payload,
    })
    .expect("failed to publish ai usage");
}

struct Shared {
    records: Mutex<Vec<(String, Value)>>,
    wake: (Mutex<bool>, Condvar),
}

pub struct AiUsageProvider {
    shared: Arc<Shared>,
    poll_secs: Duration,
}

impl AiUsageProvider {
    pub fn new(config: AiUsageConfig) -> Self {
        Self {
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
            .find(|(name, _)| name == topic)
            .map(|(_, record)| record.clone())
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
        let poll_secs = self.poll_secs;

        thread::spawn(move || {
            loop {
                publish(&shared, &tx, "ai.codex", collect_codex());
                publish(&shared, &tx, "ai.opencode", opencode_record());

                let (flag, cvar) = &shared.wake;
                let mut wake = flag.lock().unwrap();
                let deadline = std::time::Instant::now() + poll_secs;
                loop {
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

    fn run_action(&mut self, action: &str, _: Value) -> Result<ActionResult, anyhow::Error> {
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
