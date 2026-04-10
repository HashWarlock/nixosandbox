use std::collections::BTreeMap;

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentSection {
    AiCodingAgents,
    AiAssistants,
    CodeReview,
}

impl AgentSection {
    pub fn label(self) -> &'static str {
        match self {
            AgentSection::AiCodingAgents => "AI Coding Agents",
            AgentSection::AiAssistants => "AI Assistants",
            AgentSection::CodeReview => "Code Review",
        }
    }
}

pub fn classify_agent(name: &str) -> AgentSection {
    match name {
        "localgpt" | "hermes-agent" | "openclaw" => AgentSection::AiAssistants,
        "coderabbit-cli" | "tuicr" => AgentSection::CodeReview,
        _ => AgentSection::AiCodingAgents,
    }
}

pub fn grouped_agents(
    agents: &Map<String, Value>,
) -> BTreeMap<AgentSection, BTreeMap<String, Value>> {
    let mut grouped = BTreeMap::new();

    for (name, value) in agents {
        let section = classify_agent(name);
        grouped
            .entry(section)
            .or_insert_with(BTreeMap::new)
            .insert(name.clone(), value.clone());
    }

    grouped
}

fn matches_filter(name: &str, value: &Value, filter_lower: Option<&str>) -> bool {
    match filter_lower {
        None => true,
        Some(filter_lower) => {
            name.to_lowercase().contains(filter_lower)
                || value
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|d| d.to_lowercase().contains(filter_lower))
                    .unwrap_or(false)
        }
    }
}

fn filtered_entries(
    entries: &Map<String, Value>,
    filter_lower: Option<&str>,
) -> BTreeMap<String, Value> {
    entries
        .iter()
        .filter(|(name, value)| matches_filter(name, value, filter_lower))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

pub fn flat_catalog_json(catalog: &Value, filter: Option<&str>) -> Value {
    match filter {
        None => catalog.clone(),
        Some(filter) => {
            let filter_lower = filter.to_lowercase();
            let mut filtered = Map::new();
            for section in ["agents", "tools"] {
                if let Some(entries) = catalog.get(section).and_then(|v| v.as_object()) {
                    filtered.insert(
                        section.to_string(),
                        Value::Object(
                            filtered_entries(entries, Some(filter_lower.as_str()))
                                .into_iter()
                                .collect(),
                        ),
                    );
                }
            }
            Value::Object(filtered)
        }
    }
}

pub fn grouped_catalog_json(catalog: &Value, filter: Option<&str>) -> Value {
    let filter_lower = filter.map(|filter| filter.to_lowercase());
    let mut grouped = Map::new();

    if let Some(entries) = catalog.get("agents").and_then(|v| v.as_object()) {
        let grouped_agents = grouped_agents(entries);
        let mut categories = Map::new();

        for section in [
            AgentSection::AiCodingAgents,
            AgentSection::AiAssistants,
            AgentSection::CodeReview,
        ] {
            if let Some(entries) = grouped_agents.get(&section) {
                let filtered: BTreeMap<String, Value> = entries
                    .iter()
                    .filter(|(name, value)| matches_filter(name, value, filter_lower.as_deref()))
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect();
                if !filtered.is_empty() {
                    categories.insert(
                        section.label().to_string(),
                        Value::Object(filtered.into_iter().collect()),
                    );
                }
            }
        }

        grouped.insert("agentCategories".to_string(), Value::Object(categories));
    }

    if let Some(entries) = catalog.get("tools").and_then(|v| v.as_object()) {
        grouped.insert(
            "tools".to_string(),
            Value::Object(
                filtered_entries(entries, filter_lower.as_deref())
                    .into_iter()
                    .collect(),
            ),
        );
    }

    Value::Object(grouped)
}

pub fn grouped_catalog_text(catalog: &Value, filter: Option<&str>) -> String {
    let filter_lower = filter.map(|filter| filter.to_lowercase());
    let mut lines = Vec::new();

    if let Some(entries) = catalog.get("agents").and_then(|v| v.as_object()) {
        let grouped = grouped_agents(entries);
        for section in [
            AgentSection::AiCodingAgents,
            AgentSection::AiAssistants,
            AgentSection::CodeReview,
        ] {
            if let Some(entries) = grouped.get(&section) {
                lines.push(format!("{}:", section.label()));
                for (name, value) in entries {
                    if !matches_filter(name, value, filter_lower.as_deref()) {
                        continue;
                    }
                    let desc = value
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    lines.push(format!("  {:<20} {}", name, desc));
                }
                lines.push(String::new());
            }
        }
    }

    if let Some(entries) = catalog.get("tools").and_then(|v| v.as_object()) {
        lines.push("Tools (from nixpkgs):".to_string());
        for (name, value) in filtered_entries(entries, filter_lower.as_deref()) {
            let desc = value
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            lines.push(format!("  {:<20} {}", name, desc));
        }
        lines.push(String::new());
    }

    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    fn agent_map(entries: &[(&str, &str)]) -> Map<String, Value> {
        entries
            .iter()
            .map(|(name, description)| ((*name).to_string(), json!({ "description": description })))
            .collect()
    }

    #[test]
    fn groups_known_agents_into_expected_sections() {
        let agents = agent_map(&[
            ("claude-code", "Claude Code"),
            ("localgpt", "Local GPT"),
            ("coderabbit-cli", "CodeRabbit CLI"),
        ]);

        let grouped = grouped_agents(&agents);

        let coding = grouped.get(&AgentSection::AiCodingAgents).unwrap();
        assert_eq!(
            coding.keys().cloned().collect::<Vec<_>>(),
            vec!["claude-code".to_string()]
        );

        let assistants = grouped.get(&AgentSection::AiAssistants).unwrap();
        assert_eq!(
            assistants.keys().cloned().collect::<Vec<_>>(),
            vec!["localgpt".to_string()]
        );

        let review = grouped.get(&AgentSection::CodeReview).unwrap();
        assert_eq!(
            review.keys().cloned().collect::<Vec<_>>(),
            vec!["coderabbit-cli".to_string()]
        );
    }

    #[test]
    fn preserves_flat_json_shape_for_default_json_mode() {
        let catalog = json!({
            "agents": {
                "claude-code": { "description": "Claude Code" },
                "localgpt": { "description": "Local GPT" }
            },
            "tools": {
                "git": { "description": "Git" }
            }
        });

        let flat = flat_catalog_json(&catalog, None);
        assert_eq!(flat, catalog);

        let obj = flat.as_object().unwrap();
        assert!(obj.contains_key("agents"));
        assert!(obj.contains_key("tools"));
        assert_eq!(obj.len(), 2);
    }

    #[test]
    fn grouped_json_contains_agent_categories() {
        let catalog = json!({
            "agents": {
                "claude-code": { "description": "Claude Code" },
                "localgpt": { "description": "Local GPT" },
                "coderabbit-cli": { "description": "CodeRabbit CLI" }
            },
            "tools": {
                "git": { "description": "Git" }
            }
        });

        let grouped = grouped_catalog_json(&catalog, None);
        let obj = grouped.as_object().unwrap();
        assert!(obj.contains_key("agentCategories"));

        let categories = obj
            .get("agentCategories")
            .and_then(|value| value.as_object())
            .unwrap();
        assert!(categories.contains_key("AI Coding Agents"));
        assert!(categories.contains_key("AI Assistants"));
        assert!(categories.contains_key("Code Review"));

        assert_eq!(
            categories
                .get("AI Coding Agents")
                .and_then(|value| value.as_object())
                .unwrap()
                .contains_key("claude-code"),
            true
        );
        assert_eq!(
            categories
                .get("AI Assistants")
                .and_then(|value| value.as_object())
                .unwrap()
                .contains_key("localgpt"),
            true
        );
        assert_eq!(
            categories
                .get("Code Review")
                .and_then(|value| value.as_object())
                .unwrap()
                .contains_key("coderabbit-cli"),
            true
        );

        assert!(obj.contains_key("tools"));
        assert_eq!(
            obj.get("tools")
                .and_then(|value| value.as_object())
                .unwrap()
                .contains_key("git"),
            true
        );
    }
}
