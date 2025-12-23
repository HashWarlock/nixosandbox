use serde::{Deserialize, Serialize};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    #[serde(flatten)]
    pub meta: SkillMeta,
    pub body: String,
    pub scripts: Vec<String>,
    pub references: Vec<String>,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
}

// Regex for skill name validation
// Must be lowercase alphanumeric + hyphens, no consecutive hyphens, no start/end with hyphen
static SKILL_NAME_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_skill_name_regex() -> &'static Regex {
    SKILL_NAME_REGEX.get_or_init(|| {
        Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap()
    })
}

/// Validates a skill name according to agentskills.io spec:
/// - Length: 1-64 characters
/// - Characters: lowercase alphanumeric + hyphens
/// - No consecutive hyphens
/// - Cannot start or end with a hyphen
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    let len = name.len();

    if len == 0 {
        return Err("Skill name cannot be empty".to_string());
    }

    if len > 64 {
        return Err(format!("Skill name too long: {} characters (max 64)", len));
    }

    let regex = get_skill_name_regex();
    if !regex.is_match(name) {
        return Err(
            "Skill name must be lowercase alphanumeric with hyphens, \
             no consecutive hyphens, and cannot start/end with hyphen"
                .to_string(),
        );
    }

    Ok(())
}

/// Validates a skill description:
/// - Length: 1-1024 characters
pub fn validate_description(desc: &str) -> Result<(), String> {
    let len = desc.len();

    if len == 0 {
        return Err("Description cannot be empty".to_string());
    }

    if len > 1024 {
        return Err(format!("Description too long: {} characters (max 1024)", len));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_skill_name_valid() {
        assert!(validate_skill_name("my-skill").is_ok());
        assert!(validate_skill_name("skill123").is_ok());
        assert!(validate_skill_name("a").is_ok());
        assert!(validate_skill_name("my-cool-skill-2").is_ok());
    }

    #[test]
    fn test_validate_skill_name_invalid() {
        // Empty
        assert!(validate_skill_name("").is_err());

        // Too long
        let long_name = "a".repeat(65);
        assert!(validate_skill_name(&long_name).is_err());

        // Uppercase
        assert!(validate_skill_name("My-Skill").is_err());

        // Consecutive hyphens
        assert!(validate_skill_name("my--skill").is_err());

        // Start with hyphen
        assert!(validate_skill_name("-myskill").is_err());

        // End with hyphen
        assert!(validate_skill_name("myskill-").is_err());

        // Invalid characters
        assert!(validate_skill_name("my_skill").is_err());
        assert!(validate_skill_name("my.skill").is_err());
        assert!(validate_skill_name("my skill").is_err());
    }

    #[test]
    fn test_validate_description_valid() {
        assert!(validate_description("A valid description").is_ok());
        assert!(validate_description("a").is_ok());
        let long_desc = "a".repeat(1024);
        assert!(validate_description(&long_desc).is_ok());
    }

    #[test]
    fn test_validate_description_invalid() {
        // Empty
        assert!(validate_description("").is_err());

        // Too long
        let too_long = "a".repeat(1025);
        assert!(validate_description(&too_long).is_err());
    }
}
