use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub width: f32,
    pub height: f32,
    pub max_results: usize,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            width: 600.0,
            height: 420.0,
            max_results: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        role: Role,
    },
    Show {
        prompt: String,
        entries: Vec<Entry>,
        positions: HashMap<String, Position>,
        config: UiConfig,
        #[serde(default)]
        benchmark: bool,
    },
    Search {
        query: String,
    },
    SearchResults {
        query: String,
        entries: Vec<Entry>,
    },
    Position {
        screen: String,
        x: f64,
        y: f64,
    },
    Selected {
        value: String,
    },
    Presented,
    Cancelled,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Client,
    Frontend,
}
