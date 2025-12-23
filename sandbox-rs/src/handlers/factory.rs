use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::skills::{check_triggers, FactorySessions, SkillSummary};

// POST /factory/start
#[derive(Deserialize)]
pub struct StartFactoryRequest {
    pub initial_input: Option<String>,
}

// POST /factory/continue
#[derive(Deserialize)]
pub struct ContinueFactoryRequest {
    pub session_id: String,
    pub input: String,
}

// Response for start/continue
#[derive(Serialize)]
pub struct FactoryResponse {
    pub session_id: String,
    pub step: String,
    pub prompt: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillSummary>,
}

// POST /factory/check
#[derive(Deserialize)]
pub struct CheckTriggerRequest {
    pub input: String,
}

#[derive(Serialize)]
pub struct CheckTriggerResponse {
    pub triggers_factory: bool,
    pub matched_phrases: Vec<String>,
}

/// POST /factory/start - Begin dialogue
pub async fn start_factory(
    factory: &FactorySessions,
    Json(req): Json<StartFactoryRequest>,
) -> Result<Json<FactoryResponse>> {
    let session = factory.start(req.initial_input);

    Ok(Json(FactoryResponse {
        session_id: session.id,
        step: format!("{:?}", session.step),
        prompt: session.step.get_prompt().to_string(),
        done: false,
        skill: None,
    }))
}

/// POST /factory/continue - Advance step
pub async fn continue_factory(
    factory: &FactorySessions,
    Json(req): Json<ContinueFactoryRequest>,
) -> Result<Json<FactoryResponse>> {
    let session = factory
        .continue_session(&req.session_id, &req.input)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let is_done = matches!(session.step, crate::skills::factory::FactoryStep::Done);

    // Build the prompt - for Confirm step, include summary
    let prompt = if matches!(session.step, crate::skills::factory::FactoryStep::Confirm) {
        format!("{}\n\n{}", session.get_summary(), session.step.get_prompt())
    } else {
        session.step.get_prompt().to_string()
    };

    // If done, create a skill summary from the answers
    let skill = if is_done {
        let goal = session.answers.goal.as_deref().unwrap_or("Untitled Skill");
        let description = session.answers.triggers
            .as_ref()
            .map(|triggers| format!("Triggers: {}", triggers.join(", ")))
            .unwrap_or_else(|| "No triggers defined".to_string());

        Some(SkillSummary {
            name: sanitize_skill_name(goal),
            description,
        })
    } else {
        None
    };

    Ok(Json(FactoryResponse {
        session_id: session.id,
        step: format!("{:?}", session.step),
        prompt,
        done: is_done,
        skill,
    }))
}

/// POST /factory/check - Check if input triggers factory
pub async fn check_trigger(
    Json(req): Json<CheckTriggerRequest>,
) -> Result<Json<CheckTriggerResponse>> {
    let matched_phrases = check_triggers(&req.input);
    let triggers_factory = !matched_phrases.is_empty();

    Ok(Json(CheckTriggerResponse {
        triggers_factory,
        matched_phrases,
    }))
}

/// Convert a goal string into a valid skill name
/// - Convert to lowercase
/// - Replace spaces and special chars with hyphens
/// - Remove consecutive hyphens
/// - Trim hyphens from start/end
fn sanitize_skill_name(goal: &str) -> String {
    let mut name = goal
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '_' {
                '-'
            } else {
                '-'
            }
        })
        .collect::<String>();

    // Remove consecutive hyphens
    while name.contains("--") {
        name = name.replace("--", "-");
    }

    // Trim hyphens from start and end
    name = name.trim_matches('-').to_string();

    // Ensure name is not empty
    if name.is_empty() {
        name = "custom-skill".to_string();
    }

    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_skill_name() {
        assert_eq!(sanitize_skill_name("Deploy my app"), "deploy-my-app");
        assert_eq!(sanitize_skill_name("Create PDF Reports"), "create-pdf-reports");
        assert_eq!(sanitize_skill_name("Handle API@Requests"), "handle-api-requests");
        assert_eq!(sanitize_skill_name("  lots  of   spaces  "), "lots-of-spaces");
        assert_eq!(sanitize_skill_name("!!!"), "custom-skill");
        assert_eq!(sanitize_skill_name(""), "custom-skill");
    }

    #[test]
    fn test_check_trigger() {
        let req = CheckTriggerRequest {
            input: "Can you teach me how to do this?".to_string(),
        };
        let result = tokio_test::block_on(check_trigger(Json(req))).unwrap();
        assert!(result.0.triggers_factory);
        assert!(result.0.matched_phrases.contains(&"teach me".to_string()));
    }

    #[test]
    fn test_check_trigger_no_match() {
        let req = CheckTriggerRequest {
            input: "Just a regular question".to_string(),
        };
        let result = tokio_test::block_on(check_trigger(Json(req))).unwrap();
        assert!(!result.0.triggers_factory);
        assert!(result.0.matched_phrases.is_empty());
    }
}
