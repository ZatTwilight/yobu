use std::{
    fs,
    sync::{Arc, Mutex, mpsc::Sender},
    thread,
};

use serde_json::json;

use crate::{
    daemon::broker::BrokerMessage,
    providers::api::{ActionResult, ProviderTrait},
};

pub struct CpuProvider {
    last: Arc<Mutex<u64>>,
    current: Arc<Mutex<u64>>,
}

impl CpuProvider {
    pub fn new() -> Self {
        Self {
            last: Arc::new(Mutex::new(0)),
            current: Arc::new(Mutex::new(0)),
        }
    }
}

impl Default for CpuProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderTrait for CpuProvider {
    fn name(&self) -> &str {
        "cpu"
    }

    fn init(&mut self, tx: Sender<BrokerMessage>) -> std::result::Result<(), anyhow::Error> {
        let last = self.last.clone();
        let current = self.current.clone();
        let tx = tx.clone();
        let name = self.name().to_string();
        thread::spawn(move || {
            loop {
                let info = fs::read_to_string("/proc/stat").expect("Failed to read /proc/stat");
                let lines = info.lines();
                for line in lines {
                    let parts = line.split_whitespace().collect::<Vec<_>>();
                    let label = parts[0];

                    if label == "cpu" {
                        let mut last = last.lock().unwrap();
                        let mut cur = current.lock().unwrap();
                        let total: u64 = parts[1..]
                            .iter()
                            .map(|p| p.parse::<u64>().unwrap_or(0))
                            .sum();

                        let idle = parts[4].parse::<u64>().unwrap_or(0)
                            + parts[5].parse::<u64>().unwrap_or(0);
                        let diff = total - *last;
                        let idle_diff = idle - *cur;
                        let usage = (diff - idle_diff) as f64 / diff as f64 * 100.0;

                        *last = total;
                        *cur = idle;
                        println!("CPU Usage: {:.2}%", usage);
                        tx.send(BrokerMessage::Publish {
                            topic: format!("metrics.{name}"),
                            payload: json!({ "usage": usage }),
                        })
                        .expect("Failed to send CPU usage");
                    }
                }
                thread::sleep(std::time::Duration::from_secs(1));
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
    ) -> std::result::Result<ActionResult, anyhow::Error> {
        match action {
            "current" => {
                let current = self.current.lock().unwrap();
                Ok(ActionResult { result: json!({ "idle": *current }), events: vec![] })
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action)),
        }
    }
}
