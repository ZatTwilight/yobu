use std::{env, fs, path::PathBuf, time::Duration};

use chrono::Utc;
use serde_json::{Value, json};

const GO_ENDPOINT: &str = "https://opencode.ai/zen/go/v1/usage";
const GO_PROVIDERS: [&str; 2] = ["opencode", "opencode-go"];

fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/share")
        })
}

// Only account limits are collected. Never read session histories or message databases.
fn base_record(id: &str, name: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "id": id,
        "name": name,
        "updatedAt": Utc::now().to_rfc3339(),
        "ready": true,
        "hasLocalStats": false,
    })
}

pub fn codex_record() -> Value {
    base_record("codex", "Codex")
}

fn go_key() -> Option<String> {
    let auth = fs::read_to_string(data_home().join("opencode/auth.json")).ok()?;
    let auth: Value = serde_json::from_str(&auth).ok()?;
    GO_PROVIDERS
        .iter()
        .find_map(|provider| auth.get(provider)?.get("key")?.as_str().map(str::to_string))
}

fn go_limits_error(error: impl std::fmt::Display) -> Value {
    json!({
        "limits": [], "tierLabel": "Go", "usageStatusText": "Go usage unavailable",
        "authHelpText": error.to_string()
    })
}

fn go_limits() -> Value {
    let Some(key) = go_key() else {
        return json!({
            "limits": [], "tierLabel": "Go",
            "usageStatusText": "OpenCode Go sign-in required",
            "authHelpText": "Run `opencode auth login` and subscribe to Go."
        });
    };
    let response = ureq::get(GO_ENDPOINT)
        .set("Authorization", &format!("Bearer {key}"))
        .set("User-Agent", "yobu/0.1")
        .timeout(Duration::from_secs(8))
        .call();
    let body = match response {
        Ok(response) => match response.into_json::<Value>() {
            Ok(body) => body,
            Err(error) => return go_limits_error(error),
        },
        Err(error) => return go_limits_error(error),
    };
    parse_go_limits(&body)
}

fn parse_go_limits(body: &Value) -> Value {
    let windows = [
        ("5-hour", "rolling", 12.0),
        ("Weekly", "weekly", 30.0),
        ("Monthly", "monthly", 60.0),
    ];
    let limits = windows
        .into_iter()
        .filter_map(|(label, key, cap)| {
            let window = body.pointer(&format!("/usage/{key}"))?;
            let percent = window.get("percent")?.as_f64()? / 100.0;
            Some(json!({
                "label": label, "percent": percent,
                "resetsAt": window.get("resetsAt").and_then(Value::as_str).unwrap_or(""),
                "cap": cap, "currency": "USD"
            }))
        })
        .collect::<Vec<_>>();
    let status = if limits.is_empty() {
        "Go usage empty"
    } else {
        ""
    };
    json!({
        "limits": limits, "tierLabel": "Go", "usageStatusText": status,
        "authHelpText": "Run `opencode auth login` and subscribe to Go."
    })
}

pub fn opencode_record() -> Value {
    let mut record = base_record("opencode", "OpenCode Go");
    if let (Some(record), Some(limits)) = (record.as_object_mut(), go_limits().as_object()) {
        record.extend(limits.clone());
    }
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_do_not_include_history() {
        for record in [codex_record(), base_record("opencode", "OpenCode Go")] {
            assert_eq!(record["hasLocalStats"], false);
            assert_eq!(record.as_object().unwrap().len(), 6);
            for field in [
                "recentDays",
                "modelUsage",
                "todayTotalTokens",
                "totalSessions",
            ] {
                assert!(record.get(field).is_none());
            }
        }
    }

    #[test]
    fn go_limits_preserve_usage_and_reset_times() {
        let reset = "2026-09-05T12:00:00Z";
        let fields = parse_go_limits(&json!({"usage": {
            "rolling": {"percent": 25.0, "resetsAt": reset},
            "weekly": {"percent": 50.0, "resetsAt": reset},
            "monthly": {"percent": 75.0, "resetsAt": reset}
        }}));
        let limits = fields["limits"].as_array().unwrap();
        assert_eq!(limits.len(), 3);
        for (limit, percent) in limits.iter().zip([0.25, 0.5, 0.75]) {
            assert_eq!(limit["percent"], percent);
            assert_eq!(limit["resetsAt"], reset);
        }
        assert_eq!(fields["usageStatusText"], "");
    }

    #[test]
    fn empty_go_limits_have_a_status() {
        let fields = parse_go_limits(&json!({}));
        assert_eq!(fields["limits"], json!([]));
        assert_eq!(fields["usageStatusText"], "Go usage empty");
    }
}
