use serde::{Deserialize, Serialize};

pub type RequestId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Envelope {
    Hello {
        name: String,
        #[serde(default)]
        capabilities: Vec<String>,
    },
    Request {
        id: RequestId,
        method: String,
        #[serde(default)]
        params: serde_json::Value,
    },
    Response {
        id: RequestId,
        result: serde_json::Value,
    },
    Error {
        id: RequestId,
        code: String,
        message: String,
    },
    Event {
        topic: String,
        payload: serde_json::Value,
    },
    Subscribe {
        topic: String,
    },
    Unsubscribe {
        topic: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
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
