use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Show { mode: String, entries: Vec<Entry> },
    Query { id: u64, text: String },
    Activate { id: String },
    Dismiss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Results { query_id: u64, entries: Vec<Entry> },
    Selected { value: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub value: String,
}
