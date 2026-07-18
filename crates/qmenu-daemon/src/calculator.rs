use numby::{config::Config, evaluator::AgentRegistry, models::AppState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calculation {
    pub value: String,
    pub kind: String,
}

pub struct Calculator {
    registry: AgentRegistry,
    state: AppState,
}

impl Calculator {
    pub fn new() -> Result<Self, String> {
        let config = Config::default();
        let registry = AgentRegistry::new(&config).map_err(|error| error.to_string())?;
        let state = AppState::builder(&config).build();
        Ok(Self { registry, state })
    }

    pub fn calculate(&mut self, expression: &str) -> Option<Calculation> {
        if expression.trim().is_empty() {
            return None;
        }

        self.registry
            .evaluate(expression, &mut self.state)
            .map(|(value, _)| Calculation {
                value,
                kind: "Calculation".to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_arithmetic() {
        let mut calculator = Calculator::new().unwrap();
        let result = calculator.calculate("2 + 2").unwrap();
        assert_eq!(result.value, "4.00");
    }

    #[test]
    fn ignores_application_queries() {
        let mut calculator = Calculator::new().unwrap();
        assert!(calculator.calculate("firefox").is_none());
        assert!(calculator.calculate("visual studio code").is_none());
    }

    #[test]
    fn evaluates_dates_and_timezones() {
        let mut calculator = Calculator::new().unwrap();
        assert!(calculator.calculate("today + 5 days").is_some());
        assert!(calculator.calculate("time in tokyo").is_some());
    }
}
