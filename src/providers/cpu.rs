use std::{
    fs,
    sync::{Arc, Mutex, mpsc::Sender},
    thread,
    time::Duration,
};

use serde_json::json;

use crate::{
    daemon::broker::BrokerMessage,
    providers::api::{ActionResult, ProviderTrait},
};

#[derive(Clone, Copy, Default)]
struct MetricsSnapshot {
    cpu_usage: Option<f64>,
    memory_usage: Option<f64>,
}

pub struct CpuProvider {
    snapshot: Arc<Mutex<MetricsSnapshot>>,
}

impl CpuProvider {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(Mutex::new(MetricsSnapshot::default())),
        }
    }
}

impl Default for CpuProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn read_cpu_sample() -> Result<(u64, u64), anyhow::Error> {
    let info = fs::read_to_string("/proc/stat")?;
    let parts = info
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("/proc/stat is empty"))?
        .split_whitespace()
        .collect::<Vec<_>>();
    if parts.first() != Some(&"cpu") || parts.len() < 6 {
        return Err(anyhow::anyhow!("invalid aggregate CPU row"));
    }

    let values = parts[1..]
        .iter()
        .map(|part| part.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()?;
    let total = values.iter().sum();
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    Ok((total, idle))
}

fn read_memory_usage() -> Result<f64, anyhow::Error> {
    let info = fs::read_to_string("/proc/meminfo")?;
    let mut total = None;
    let mut available = None;
    for line in info.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total = parts.next().and_then(|value| value.parse::<u64>().ok()),
            Some("MemAvailable:") => {
                available = parts.next().and_then(|value| value.parse::<u64>().ok())
            }
            _ => {}
        }
    }

    let total = total.ok_or_else(|| anyhow::anyhow!("MemTotal missing"))?;
    let available = available.ok_or_else(|| anyhow::anyhow!("MemAvailable missing"))?;
    if total == 0 {
        return Err(anyhow::anyhow!("MemTotal is zero"));
    }
    Ok((total.saturating_sub(available)) as f64 / total as f64 * 100.0)
}

impl ProviderTrait for CpuProvider {
    fn name(&self) -> &str {
        "cpu"
    }

    fn init(&mut self, tx: Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
        let snapshot = self.snapshot.clone();
        thread::spawn(move || {
            let mut previous_cpu: Option<(u64, u64)> = None;
            loop {
                let cpu_usage = match read_cpu_sample() {
                    Ok((total, idle)) => {
                        let usage = previous_cpu.and_then(|(previous_total, previous_idle)| {
                            let total_delta = total.saturating_sub(previous_total);
                            let idle_delta = idle.saturating_sub(previous_idle);
                            (total_delta > 0).then(|| {
                                (1.0 - idle_delta.min(total_delta) as f64 / total_delta as f64)
                                    * 100.0
                            })
                        });
                        previous_cpu = Some((total, idle));
                        usage
                    }
                    Err(error) => {
                        eprintln!("metrics CPU read failed: {error}");
                        None
                    }
                };

                let memory_usage = read_memory_usage()
                    .map_err(|error| {
                        eprintln!("metrics memory read failed: {error}");
                        error
                    })
                    .ok();

                {
                    let mut current = snapshot.lock().unwrap();
                    if cpu_usage.is_some() {
                        current.cpu_usage = cpu_usage;
                    }
                    if memory_usage.is_some() {
                        current.memory_usage = memory_usage;
                    }
                }

                if let Some(usage) = cpu_usage
                    && tx
                        .send(BrokerMessage::Publish {
                            topic: "metrics.cpu".into(),
                            payload: json!({ "usage": usage }),
                        })
                        .is_err()
                {
                    return;
                }
                if let Some(usage) = memory_usage
                    && tx
                        .send(BrokerMessage::Publish {
                            topic: "metrics.memory".into(),
                            payload: json!({ "usage": usage }),
                        })
                        .is_err()
                {
                    return;
                }

                thread::sleep(Duration::from_secs(1));
            }
        });
        Ok(())
    }

    fn get_actions(&self) -> Vec<String> {
        vec!["current".into()]
    }

    fn run_action(
        &mut self,
        action: &str,
        _: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            "current" => {
                let current = *self.snapshot.lock().unwrap();
                Ok(ActionResult {
                    result: json!({
                        "cpu": { "usage": current.cpu_usage },
                        "memory": { "usage": current.memory_usage },
                    }),
                    events: vec![],
                })
            }
            _ => Err(anyhow::anyhow!("unknown CPU action: {action}")),
        }
    }
}
