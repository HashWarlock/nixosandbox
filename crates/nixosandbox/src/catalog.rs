use std::collections::BTreeMap;

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentSection {
    AiCodingAgents,
    AiAssistants,
    CodeReview,
    Other,
}

impl AgentSection {
    pub fn label(self) -> &'static str {
        match self {
            AgentSection::AiCodingAgents => "AI Coding Agents",
            AgentSection::AiAssistants => "AI Assistants",
            AgentSection::CodeReview => "Code Review",
            AgentSection::Other => "Other Agents",
        }
    }
}

pub fn classify_agent(name: &str) -> AgentSection {
    let lower = name.to_ascii_lowercase();

    if lower.contains("review") {
        AgentSection::CodeReview
    } else if matches!(lower.as_str(), "claude-code" | "codex" | "opencode" | "aider") {
        AgentSection::AiCodingAgents
    } else if matches!(lower.as_str(), "pi" | "gemini" | "assistant") {
        AgentSection::AiAssistants
    } else {
        AgentSection::Other
    }
}

pub fn grouped_agents(agents: &Map<String, Value>) -> BTreeMap<AgentSection, BTreeMap<String, Value>> {
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

pub fn flat_catalog_json(catalog: &Value) -> Value {
    catalog.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    fn agent_map(entries: &[(&str, &str)]) -> Map<String, Value> {
        entries
            .iter()
            .map(|(name, description)| {
                (
                    (*name).to_string(),
                    json!({ "description": description }),
                )
            })
            .collect()
    }

    #[test]
    fn groups_known_agents_into_expected_sections() {
        let agents = agent_map(&[
            ("claude-code", "Claude Code"),
            ("pi", "Pi assistant"),
            ("codex", "Codex"),
            ("review", "Review helper"),
        ]);

        let grouped = grouped_agents(&agents);

        let coding = grouped.get(&AgentSection::AiCodingAgents).unwrap();
        assert_eq!(
            coding.keys().cloned().collect::<Vec<_>>(),
            vec!["claude-code".to_string(), "codex".to_string()]
        );

        let assistants = grouped.get(&AgentSection::AiAssistants).unwrap();
        assert_eq!(
            assistants.keys().cloned().collect::<Vec<_>>(),
            vec!["pi".to_string()]
        );

        let review = grouped.get(&AgentSection::CodeReview).unwrap();
        assert_eq!(
            review.keys().cloned().collect::<Vec<_>>(),
            vec!["review".to_string()]
        );
    }

    #[test]
    fn preserves_flat_json_shape_for_default_json_mode() {
        let catalog = json!({
            "agents": {
                "claude-code": { "description": "Claude Code" },
                "pi": { "description": "Pi assistant" }
            },
            "tools": {
                "git": { "description": "Git" }
            }
        });

        let flat = flat_catalog_json(&catalog);
        assert_eq!(flat, catalog);

        let obj = flat.as_object().unwrap();
        assert!(obj.contains_key("agents"));
        assert!(obj.contains_key("tools"));
        assert_eq!(obj.len(), 2);
    }
}
