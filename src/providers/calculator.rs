use serde::Deserialize;
use serde_json::json;

use crate::{
    calculator::{Calculation, Calculator},
    daemon::broker::BrokerMessage,
    protocol::Entry,
    providers::api::{ActionResult, ProviderTrait},
};

#[derive(Deserialize)]
struct EvaluateParams {
    query: String,
}

pub struct CalculatorProvider {
    calculator: Option<Calculator>,
}

impl CalculatorProvider {
    pub fn new() -> Self {
        let calculator = match Calculator::new() {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("Numby unavailable; calculator disabled: {e}");
                None
            }
        };
        Self { calculator }
    }
}

impl Default for CalculatorProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn calculation_entry(query: &str, calc: Calculation) -> Entry {
    Entry {
        id: "calculation".into(),
        label: calc.value.clone(),
        description: Some(format!("{} · {query}", calc.kind)),
        icon: Some("accessories-calculator".into()),
        preview: None,
        value: calc.value,
    }
}

impl ProviderTrait for CalculatorProvider {
    fn name(&self) -> &str {
        "calculator"
    }

    fn init(&mut self, _: std::sync::mpsc::Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn get_actions(&self) -> Vec<String> {
        vec!["evaluate".into()]
    }

    fn run_action(
        &mut self,
        action: &str,
        params: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            "evaluate" => {
                let p: EvaluateParams = serde_json::from_value(params)?;
                let query = p.query.trim().to_owned();
                if query.is_empty() {
                    return Ok(ActionResult {
                        result: json!({ "query": p.query, "entry": null }),
                        events: Vec::new(),
                    });
                }
                let entry = self
                    .calculator
                    .as_mut()
                    .and_then(|c| c.calculate(&query))
                    .map(|calc| calculation_entry(&query, calc));
                Ok(ActionResult {
                    result: json!({ "query": p.query, "entry": entry }),
                    events: Vec::new(),
                })
            }
            _ => Err(anyhow::anyhow!("unknown calculator action: {action}")),
        }
    }
}
