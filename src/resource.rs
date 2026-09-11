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
        build_live_output(resource_id)?
    };
    println!("{}", serde_json::to_string(&output).map_err(|e| e.to_string())?);
    Ok(())
}

fn build_mock_output(resource_id: &str) -> Result<ResourceOutput, String> {
    if mock_scenario() == "malformed" {
        println!("not-json");
        std::process::exit(0);
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

fn build_live_output(resource_id: &str) -> Result<ResourceOutput, String> {
    let cfg = Config::load()?;
    if !cfg.sidebar.enabled {
        return Ok(ResourceOutput {
            schema: RESOURCE_SCHEMA,
            plugin_id: "herdr-jira",
            resource_id: resource_id.to_string(),
            label: cfg.sidebar_title(),
            items: vec![],
            matched_count: Some(0),
            truncated: false,
            fetched_unix_ms: now_ms(),
            error: None,
        });
    }
    let client = JiraClient::new(&cfg)?;
    let jql = cfg.sidebar_jql();
    let result = client.search_with_meta(&jql)?;
    Ok(to_output(resource_id, &cfg.sidebar_title(), result, None))
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
                payload: issue.key.clone(),
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
