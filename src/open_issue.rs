//! Activate a sidebar issue in the Jira pane (`herdr-jira open-issue`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUEST_FILE: &str = "open-issue.json";
const SEQ_FILE: &str = "open-issue.seq";

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenIssueRequest {
    pub seq: u64,
    pub key: String,
}

pub fn run() -> Result<(), String> {
    let key = issue_key_from_env()?;
    write_request(&key)?;
    focus_or_open_jira_pane()?;
    Ok(())
}

fn issue_key_from_env() -> Result<String, String> {
    for name in ["HERDR_PLUGIN_ITEM_PAYLOAD", "HERDR_PLUGIN_ITEM_ID"] {
        if let Ok(value) = std::env::var(name) {
            let key = value.trim();
            if !key.is_empty() {
                return Ok(key.to_string());
            }
        }
    }
    Err("open-issue requires HERDR_PLUGIN_ITEM_ID or HERDR_PLUGIN_ITEM_PAYLOAD".into())
}

pub fn state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("HERDR_PLUGIN_STATE_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/state/herdr/plugins/state/herdr-jira")
}

pub fn write_request(key: &str) -> Result<(), String> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create plugin state dir: {error}"))?;
    let seq = next_seq(&dir)?;
    let request = OpenIssueRequest {
        seq,
        key: key.to_string(),
    };
    let path = dir.join(REQUEST_FILE);
    let content = serde_json::to_string(&request).map_err(|error| error.to_string())?;
    let temp = dir.join(format!(
        ".open-issue.tmp-{}-{}",
        std::process::id(),
        seq
    ));
    std::fs::write(&temp, content)
        .map_err(|error| format!("failed to write open-issue request: {error}"))?;
    std::fs::rename(&temp, &path)
        .map_err(|error| format!("failed to replace open-issue request: {error}"))?;
    Ok(())
}

pub fn read_latest_request() -> Option<OpenIssueRequest> {
    let path = state_dir().join(REQUEST_FILE);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn next_seq(dir: &Path) -> Result<u64, String> {
    let path = dir.join(SEQ_FILE);
    let current = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let next = current + 1;
    std::fs::write(&path, next.to_string())
        .map_err(|error| format!("failed to update open-issue sequence: {error}"))?;
    Ok(next)
}

fn focus_or_open_jira_pane() -> Result<(), String> {
    let herdr = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into());
    if Command::new(&herdr)
        .args([
            "plugin",
            "pane",
            "focus",
            "--plugin",
            "herdr-jira",
            "--entrypoint",
            "jira",
        ])
        .status()
        .map_err(|error| format!("failed to run herdr: {error}"))?
        .success()
    {
        return Ok(());
    }
    let status = Command::new(&herdr)
        .args([
            "plugin",
            "pane",
            "open",
            "--plugin",
            "herdr-jira",
            "--entrypoint",
            "jira",
            "--placement",
            "split",
            "--direction",
            "right",
            "--focus",
        ])
        .status()
        .map_err(|error| format!("failed to run herdr: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("herdr plugin pane open exited with {status}"))
    }
}
