use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    config::{self, SavedState},
    daemon::broker::BrokerMessage,
    protocol::{Position, UiConfig},
    providers::api::{ActionResult, ProviderEvent, ProviderTrait},
};

#[derive(Debug, Deserialize)]
struct SetPositionParams {
    screen: String,
    x: f64,
    y: f64,
}

#[derive(Debug, Serialize)]
struct StateSnapshot {
    positions: std::collections::HashMap<String, Position>,
    config: UiConfig,
}

pub struct StateProvider {
    state: Arc<Mutex<SavedState>>,
}

impl StateProvider {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(config::load_state())),
        }
    }

    pub fn with_shared(state: Arc<Mutex<SavedState>>) -> Self {
        Self { state }
    }

    fn load_config() -> UiConfig {
        config::load_config()
    }
}

impl Default for StateProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderTrait for StateProvider {
    fn name(&self) -> &str {
        "state"
    }

    fn init(&mut self, _: std::sync::mpsc::Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn get_actions(&self) -> Vec<String> {
        vec!["get".into(), "set_position".into(), "get_config".into()]
    }

    fn run_action(
        &mut self,
        action: &str,
        params: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            "get" => {
                let state = self.state.lock().unwrap();
                Ok(ActionResult {
                    result: json!(StateSnapshot {
                        positions: state.positions.clone(),
                        config: Self::load_config(),
                    }),
                    events: Vec::new(),
                })
            }
            "get_config" => Ok(ActionResult {
                result: json!(Self::load_config()),
                events: Vec::new(),
            }),
            "set_position" => {
                let p: SetPositionParams = serde_json::from_value(params)?;
                {
                    let mut state = self.state.lock().unwrap();
                    state
                        .positions
                        .insert(p.screen.clone(), Position { x: p.x, y: p.y });
                    // Persist synchronously so a crash right after drag still saves.
                    if let Err(e) = config::save_state(&state) {
                        eprintln!("failed to save position: {e}");
                    }
                }
                Ok(ActionResult {
                    result: json!({ "ok": true }),
                    events: vec![ProviderEvent {
                        topic: "state.positions".into(),
                        payload: json!({ "screen": p.screen, "x": p.x, "y": p.y }),
                    }],
                })
            }
            _ => Err(anyhow::anyhow!("unknown state action: {action}")),
        }
    }
}
