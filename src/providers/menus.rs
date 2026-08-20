use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::json;

use crate::{
    daemon::broker::BrokerMessage,
    protocol::Entry,
    providers::api::{ActionResult, ProviderTrait},
};

const SESSION_TTL: Duration = Duration::from_secs(10 * 60);

struct MenuSession {
    prompt: String,
    entries: Vec<Entry>,
    result: Option<Option<String>>,
    created: Instant,
}

#[derive(Deserialize)]
struct CreateParams {
    prompt: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct KeyParams {
    key: String,
}
#[derive(Deserialize)]
struct CompleteParams {
    key: String,
    value: Option<String>,
}

pub struct MenusProvider {
    sessions: HashMap<String, MenuSession>,
    next_id: u64,
}

impl MenusProvider {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    fn cleanup(&mut self) {
        self.sessions
            .retain(|_, session| session.created.elapsed() < SESSION_TTL);
    }

    fn new_key(&mut self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = self.next_id;
        self.next_id += 1;
        format!("{now:x}-{id:x}")
    }
}

impl Default for MenusProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderTrait for MenusProvider {
    fn name(&self) -> &str {
        "menus"
    }
    fn init(&mut self, _: std::sync::mpsc::Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
        Ok(())
    }
    fn get_actions(&self) -> Vec<String> {
        vec![
            "create".into(),
            "get".into(),
            "complete".into(),
            "take_result".into(),
        ]
    }

    fn run_action(
        &mut self,
        action: &str,
        params: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        self.cleanup();
        let result = match action {
            "create" => {
                let p: CreateParams = serde_json::from_value(params)?;
                let key = self.new_key();
                self.sessions.insert(
                    key.clone(),
                    MenuSession {
                        prompt: p.prompt,
                        entries: p.entries,
                        result: None,
                        created: Instant::now(),
                    },
                );
                json!({ "key": key })
            }
            "get" => {
                let p: KeyParams = serde_json::from_value(params)?;
                let session = self
                    .sessions
                    .get(&p.key)
                    .ok_or_else(|| anyhow::anyhow!("menu session not found"))?;
                json!({ "prompt": session.prompt, "entries": session.entries })
            }
            "complete" => {
                let p: CompleteParams = serde_json::from_value(params)?;
                let session = self
                    .sessions
                    .get_mut(&p.key)
                    .ok_or_else(|| anyhow::anyhow!("menu session not found"))?;
                session.result = Some(p.value);
                json!({ "ok": true })
            }
            "take_result" => {
                let p: KeyParams = serde_json::from_value(params)?;
                let Some(session) = self.sessions.get(&p.key) else {
                    return Err(anyhow::anyhow!("menu session not found"));
                };
                if session.result.is_none() {
                    json!({ "done": false })
                } else {
                    let session = self.sessions.remove(&p.key).unwrap();
                    json!({ "done": true, "value": session.result.unwrap() })
                }
            }
            _ => return Err(anyhow::anyhow!("unknown menus action: {action}")),
        };
        Ok(ActionResult {
            result,
            events: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_session_completes_and_is_removed() {
        let mut provider = MenusProvider::new();
        let created = provider
            .run_action("create", json!({
                "prompt": "Pick",
                "entries": [{ "id": "0", "label": "Alpha", "description": null, "icon": null, "value": "Alpha" }]
            }))
            .unwrap()
            .result;
        let key = created["key"].as_str().unwrap();

        let pending = provider
            .run_action("take_result", json!({ "key": key }))
            .unwrap()
            .result;
        assert_eq!(pending["done"], false);

        provider
            .run_action("complete", json!({ "key": key, "value": "Alpha" }))
            .unwrap();
        let completed = provider
            .run_action("take_result", json!({ "key": key }))
            .unwrap()
            .result;
        assert_eq!(completed, json!({ "done": true, "value": "Alpha" }));
        assert!(provider.run_action("get", json!({ "key": key })).is_err());
    }
}
