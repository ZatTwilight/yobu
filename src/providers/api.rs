use std::sync::mpsc::Sender;

use crate::daemon::broker::BrokerMessage;

pub struct ActionResult {
    pub result: serde_json::Value,
    pub events: Vec<ProviderEvent>,
}

pub struct ProviderEvent {
    pub topic: String,
    pub payload: serde_json::Value,
}

pub trait ProviderTrait: Send {
    fn name(&self) -> &str;

    fn init(&mut self, broker_tx: Sender<BrokerMessage>) -> Result<(), anyhow::Error>;

    fn get_actions(&self) -> Vec<String>;

    fn run_action(
        &mut self,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error>;
}
