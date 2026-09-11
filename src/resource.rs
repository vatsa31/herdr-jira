//! Headless sidebar resource provider (`herdr-jira resource --id …`).

use crate::config::Config;
use crate::jira::JiraClient;
use crate::mock::{mock_enabled, mock_scenario, mock_search, RESOURCE_SCHEMA};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct ResourceOutput {
    schema: &'static str,
    plugin_id: &'static str,
    resource_id: String,
    label: String,
    items: Vec<ResourceItem>,
    matched_count: Option<u64>,
    truncated: bool,
    fetched_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct ResourceItem {
    id: String,
    primary: String,
    secondary: String,
    payload: String,
}

pub fn run(resource_id: &str, force_mock: bool) -> Result<(), String> {
    if resource_id != "my-issues" {
        return Err(format!("unknown resource id '{resource_id}'"));
    }
    let mock = force_mock || mock_enabled();
    let output = if mock {
        build_mock_output(resource_id)?
    } else {
        build_live_output(resource_id)
    };
    println!("{}", serde_json::to_string(&output).map_err(|e| e.to_string())?);
    Ok(())
}

fn error_output(resource_id: &str, label: String, error: String) -> ResourceOutput {
    ResourceOutput {
        schema: RESOURCE_SCHEMA,
        plugin_id: "herdr-jira",
        resource_id: resource_id.to_string(),
        label,
        items: vec![],
        matched_count: None,
        truncated: false,
        fetched_unix_ms: now_ms(),
        error: Some(error),
    }
}

fn build_mock_output(resource_id: &str) -> Result<ResourceOutput, String> {
    if mock_scenario() == "malformed" {
        println!("not-json");
        std::process::exit(0);
    }
    if mock_scenario() == "delayed" {
        // Exceeds the Herdr-side 25s provider timeout so timeout handling can
        // be exercised without Jira credentials.
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
    let cfg = Config::load().unwrap_or_else(|_| Config::default_mock());
    let label = cfg.sidebar_title();
    match mock_search(mock_scenario()) {
        Ok(result) => Ok(to_output(resource_id, &label, result, None)),
        Err(error) => Ok(ResourceOutput {
            schema: RESOURCE_SCHEMA,
            plugin_id: "herdr-jira",
            resource_id: resource_id.to_string(),
            label,
            items: vec![],
            matched_count: None,
            truncated: false,
            fetched_unix_ms: now_ms(),
            error: Some(error),
        }),
    }
}

fn build_live_output(resource_id: &str) -> ResourceOutput {
    let cfg = match Config::load() {
        Ok(cfg) => cfg,
        Err(error) => {
            let label = Config::default_mock().sidebar_title();
            return error_output(resource_id, label, sanitize(&error));
        }
    };
    if !cfg.sidebar.enabled {
        return ResourceOutput {
            schema: RESOURCE_SCHEMA,
            plugin_id: "herdr-jira",
            resource_id: resource_id.to_string(),
            label: cfg.sidebar_title(),
            items: vec![],
            matched_count: Some(0),
            truncated: false,
            fetched_unix_ms: now_ms(),
            error: Some("sidebar section disabled in plugin config".into()),
        };
    }
    let label = cfg.sidebar_title();
    let client = match JiraClient::new(&cfg) {
        Ok(client) => client,
        Err(error) => return error_output(resource_id, label, sanitize(&error)),
    };
    let jql = cfg.sidebar_jql();
    match client.search_with_meta(&jql) {
        Ok(result) => to_output(resource_id, &label, result, None),
        Err(error) => error_output(resource_id, label, sanitize(&error)),
    }
}

fn to_output(
    resource_id: &str,
    label: &str,
    result: crate::jira::SearchResult,
    error: Option<String>,
) -> ResourceOutput {
    ResourceOutput {
        schema: RESOURCE_SCHEMA,
        plugin_id: "herdr-jira",
        resource_id: resource_id.to_string(),
        label: label.to_string(),
        items: result
            .issues
            .iter()
            .map(|issue| ResourceItem {
                id: sanitize(&issue.key),
                primary: sanitize(&issue.summary),
                secondary: sanitize(&issue.status),
                payload: sanitize(&issue.key),
            })
            .collect(),
        matched_count: result.matched_count,
        truncated: result.truncated,
        fetched_unix_ms: now_ms(),
        error,
    }
}

fn sanitize(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jira::{Issue, SearchResult};

    fn issue(key: &str, summary: &str, status: &str) -> Issue {
        Issue {
            key: key.into(),
            summary: summary.into(),
            status: status.into(),
            status_category: "indeterminate".into(),
            issue_type: "Task".into(),
            priority: "Medium".into(),
            assignee: "Mock".into(),
            reporter: "Mock".into(),
            updated: "2026-09-11".into(),
            labels: vec![],
            description: String::new(),
            url: String::new(),
        }
    }

    #[test]
    fn sanitize_strips_controls_and_collapses_whitespace() {
        assert_eq!(sanitize("a\x1bb [\x1b[31m c"), "a b [ [31m c");
        assert_eq!(sanitize("  a\t\n b  "), "a b");
        assert_eq!(sanitize("MOCK-1"), "MOCK-1");
    }

    #[test]
    fn to_output_sanitizes_payload_and_reports_truncation() {
        let result = SearchResult {
            issues: vec![issue("MOCK-\x1b1", "sum\x07mary", "To Do")],
            matched_count: Some(128),
            truncated: true,
        };
        let out = to_output("my-issues", "My Jira issues", result, None);
        assert_eq!(out.items[0].payload, "MOCK- 1");
        assert_eq!(out.items[0].primary, "sum mary");
        assert!(out.truncated);
        assert_eq!(out.matched_count, Some(128));
    }

    #[test]
    fn error_output_carries_error_with_empty_items() {
        let out = error_output("my-issues", "My Jira issues".into(), "boom".into());
        assert!(out.items.is_empty());
        assert_eq!(out.error.as_deref(), Some("boom"));
        assert_eq!(out.schema, RESOURCE_SCHEMA);
    }
}
