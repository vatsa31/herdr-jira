//! Deterministic mock Jira issues for sidebar/provider development without credentials.

use crate::jira::{Issue, SearchResult};

pub const RESOURCE_SCHEMA: &str = "herdr.plugin.resource/v1";

pub fn mock_enabled() -> bool {
    std::env::var("HERDR_JIRA_MOCK")
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

pub fn mock_scenario() -> &'static str {
    match std::env::var("HERDR_JIRA_MOCK_SCENARIO")
        .unwrap_or_default()
        .as_str()
    {
        "empty" => "empty",
        "long" => "long",
        "unicode" => "unicode",
        "failure" => "failure",
        "malformed" => "malformed",
        "recovery" => "recovery",
        "reorder" => "reorder",
        _ => "few",
    }
}

pub fn mock_search(scenario: &str) -> Result<SearchResult, String> {
    match scenario {
        "empty" => Ok(SearchResult {
            issues: vec![],
            matched_count: Some(0),
            truncated: false,
        }),
        "failure" => Err("mock Jira provider failure".into()),
        "few" => Ok(SearchResult {
            issues: vec![
                issue("MOCK-142", "Fix login timeout", "In Progress"),
                issue("MOCK-137", "Add export option", "To Do"),
                issue("MOCK-120", "Update onboarding copy", "In Review"),
            ],
            matched_count: Some(3),
            truncated: false,
        }),
        "long" => {
            let issues = (1..=40)
                .map(|n| issue(format!("MOCK-{n:03}"), format!("Synthetic issue {n}"), "To Do"))
                .collect();
            Ok(SearchResult {
                issues,
                matched_count: Some(128),
                truncated: true,
            })
        }
        "unicode" => Ok(SearchResult {
            issues: vec![
                issue("MOCK-日本", "ログイン改善 🚀", "進行中"),
                issue("MOCK-Ω", "Résumé très long pour tester la troncature", "À faire"),
            ],
            matched_count: Some(2),
            truncated: false,
        }),
        "recovery" | "reorder" => Ok(SearchResult {
            issues: vec![
                issue("MOCK-201", "Reorder check alpha", "Done"),
                issue("MOCK-142", "Fix login timeout", "In Progress"),
                issue("MOCK-999", "Newly arrived issue", "To Do"),
            ],
            matched_count: Some(3),
            truncated: false,
        }),
        other => Err(format!("unknown mock scenario '{other}'")),
    }
}

fn issue(key: impl Into<String>, summary: impl Into<String>, status: impl Into<String>) -> Issue {
    let key = key.into();
    Issue {
        key: key.clone(),
        summary: summary.into(),
        status: status.into(),
        status_category: "indeterminate".into(),
        issue_type: "Task".into(),
        priority: "Medium".into(),
        assignee: "Mock User".into(),
        reporter: "Mock User".into(),
        updated: "2026-09-11 09:00".into(),
        labels: vec![],
        description: String::new(),
        url: format!("https://mock.example/browse/{key}"),
    }
}
