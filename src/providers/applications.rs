use serde_json::json;

use crate::{
    apps::Applications,
    daemon::broker::BrokerMessage,
    providers::api::{ActionResult, ProviderEvent, ProviderTrait},
};

pub struct ApplicationsProvider {
    applications: Applications,
}

impl ApplicationsProvider {
    pub fn new() -> Self {
        Self {
            applications: Applications::discover(),
        }
    }
}

impl Default for ApplicationsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderTrait for ApplicationsProvider {
    fn name(&self) -> &str {
        "applications"
    }

    fn init(&mut self, _: std::sync::mpsc::Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn get_actions(&self) -> Vec<String> {
        vec!["list".into(), "refresh".into()]
    }

    fn run_action(
        &mut self,
        action: &str,
        _: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            "list" => Ok(ActionResult {
                result: json!({ "entries": &self.applications.entries }),
                events: Vec::new(),
            }),
            "refresh" => {
                self.applications = Applications::discover();
                let count = self.applications.entries.len();
                Ok(ActionResult {
                    result: json!({ "count": count }),
                    events: vec![ProviderEvent {
                        topic: "applications.changed".into(),
                        payload: json!({ "count": count }),
                    }],
                })
            }
            _ => Err(anyhow::anyhow!("unknown applications action: {action}")),
        }
    }
}
