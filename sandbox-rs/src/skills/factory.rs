use dashmap::DashMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum FactoryStep {
    Goal,
    Trigger,
    Example,
    Complexity,
    EdgeCases,
    Confirm,
    Done,
}

impl FactoryStep {
    /// Get the prompt message for the current step
    pub fn get_prompt(&self) -> &'static str {
        match self {
            FactoryStep::Goal => "What task do you want me to help with? Give me the high-level goal.",
            FactoryStep::Trigger => "When should I use this skill? What words or situations should activate it?",
            FactoryStep::Example => "Walk me through a real example. What would you give me as input, and what should I produce?",
            FactoryStep::Complexity => "Is this a simple skill (text instructions only) or complex (needs scripts, templates)?",
            FactoryStep::EdgeCases => "What should I do if something's missing or goes wrong?",
            FactoryStep::Confirm => "Does this capture what you want? Say 'yes' to create.",
            FactoryStep::Done => "Skill creation complete!",
        }
    }

    /// Get the next step in the workflow
    pub fn next(&self) -> Self {
        match self {
            FactoryStep::Goal => FactoryStep::Trigger,
            FactoryStep::Trigger => FactoryStep::Example,
            FactoryStep::Example => FactoryStep::Complexity,
            FactoryStep::Complexity => FactoryStep::EdgeCases,
            FactoryStep::EdgeCases => FactoryStep::Confirm,
            FactoryStep::Confirm => FactoryStep::Done,
            FactoryStep::Done => FactoryStep::Done,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FactoryAnswers {
    pub goal: Option<String>,
    pub triggers: Option<Vec<String>>,
    pub example_input: Option<String>,
    pub example_output: Option<String>,
    pub complexity: Option<Complexity>,
    pub edge_cases: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Complexity {
    Simple,
    Complex,
}

#[derive(Debug, Clone)]
pub struct FactorySession {
    pub id: String,
    pub step: FactoryStep,
    pub answers: FactoryAnswers,
    #[allow(dead_code)] // Used by cleanup_expired
    pub created_at: Instant,
}

impl FactorySession {
    /// Create a new factory session
    pub fn new(id: String, initial_input: Option<String>) -> Self {
        let mut answers = FactoryAnswers::default();
        let step = if let Some(input) = initial_input {
            answers.goal = Some(input);
            FactoryStep::Trigger
        } else {
            FactoryStep::Goal
        };

        Self {
            id,
            step,
            answers,
            created_at: Instant::now(),
        }
    }

    /// Get a summary of the current session for confirmation
    pub fn get_summary(&self) -> String {
        let goal = self.answers.goal.as_deref().unwrap_or("(not specified)");
        let triggers = self.answers.triggers.as_ref()
            .map(|t| t.join(", "))
            .unwrap_or_else(|| "(not specified)".to_string());
        let example_input = self.answers.example_input.as_deref().unwrap_or("(not specified)");
        let example_output = self.answers.example_output.as_deref().unwrap_or("(not specified)");
        let complexity = match &self.answers.complexity {
            Some(Complexity::Simple) => "Simple (text instructions only)",
            Some(Complexity::Complex) => "Complex (needs scripts/templates)",
            None => "(not specified)",
        };
        let edge_cases = self.answers.edge_cases.as_deref().unwrap_or("(not specified)");

        format!(
            "# Skill Summary\n\n\
            **Goal:** {}\n\
            **Triggers:** {}\n\
            **Example Input:** {}\n\
            **Example Output:** {}\n\
            **Complexity:** {}\n\
            **Edge Cases:** {}\n",
            goal, triggers, example_input, example_output, complexity, edge_cases
        )
    }
}

#[derive(Clone)]
pub struct FactorySessions {
    sessions: DashMap<String, FactorySession>,
}

impl FactorySessions {
    /// Create a new factory sessions manager
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Start a new factory session
    pub fn start(&self, initial_input: Option<String>) -> FactorySession {
        let id = uuid::Uuid::new_v4().to_string();
        let session = FactorySession::new(id.clone(), initial_input);
        self.sessions.insert(id, session.clone());
        session
    }

    /// Continue an existing session with user input
    pub fn continue_session(&self, id: &str, input: &str) -> anyhow::Result<FactorySession> {
        let mut session = self.sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

        // Process input based on current step
        match session.step {
            FactoryStep::Goal => {
                session.answers.goal = Some(input.to_string());
                session.step = session.step.next();
            }
            FactoryStep::Trigger => {
                // Parse triggers from input (split by commas, newlines, or semicolons)
                let triggers: Vec<String> = input
                    .split(&[',', '\n', ';'][..])
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                session.answers.triggers = Some(triggers);
                session.step = session.step.next();
            }
            FactoryStep::Example => {
                // Parse example input/output from various formats:
                // 1. "input: X -> output: Y" or "input: X output: Y"
                // 2. "X -> Y" (arrow separator)
                // 3. Just "X" (no separator, only input)

                let input_lower = input.to_lowercase();

                // Try to find "input:" and "output:" markers
                if let Some(input_pos) = input_lower.find("input:") {
                    let after_input = &input[input_pos + 6..];

                    if let Some(output_pos) = input_lower.find("output:") {
                        // Both markers found
                        let input_text = if output_pos > input_pos + 6 {
                            input[input_pos + 6..output_pos].trim()
                        } else {
                            after_input.trim()
                        };
                        let output_text = input[output_pos + 7..].trim();

                        session.answers.example_input = Some(input_text.to_string());
                        session.answers.example_output = if !output_text.is_empty() {
                            Some(output_text.to_string())
                        } else {
                            None
                        };
                    } else {
                        // Only input marker
                        session.answers.example_input = Some(after_input.trim().to_string());
                        session.answers.example_output = None;
                    }
                } else if let Some(arrow_pos) = input.find("->") {
                    // Try arrow separator
                    let input_part = input[..arrow_pos].trim();
                    let output_part = input[arrow_pos + 2..].trim();

                    session.answers.example_input = Some(input_part.to_string());
                    session.answers.example_output = if !output_part.is_empty() {
                        Some(output_part.to_string())
                    } else {
                        None
                    };
                } else {
                    // No separator found, store whole input as example_input
                    session.answers.example_input = Some(input.to_string());
                    session.answers.example_output = None;
                }

                session.step = session.step.next();
            }
            FactoryStep::Complexity => {
                let normalized = input.trim().to_lowercase();
                let complexity = if normalized.contains("simple") || normalized.contains("text") {
                    Complexity::Simple
                } else if normalized.contains("complex") || normalized.contains("script") || normalized.contains("template") {
                    Complexity::Complex
                } else {
                    // Default to simple if unclear
                    Complexity::Simple
                };
                session.answers.complexity = Some(complexity);
                session.step = session.step.next();
            }
            FactoryStep::EdgeCases => {
                session.answers.edge_cases = Some(input.to_string());
                session.step = session.step.next();
            }
            FactoryStep::Confirm => {
                let normalized = input.trim().to_lowercase();
                if normalized == "yes" || normalized == "y" || normalized == "confirm" {
                    session.step = FactoryStep::Done;
                } else {
                    // Reset to Goal step but preserve answers for review/modification
                    session.step = FactoryStep::Goal;
                }
            }
            FactoryStep::Done => {
                // Already done, no changes
            }
        }

        Ok(session.clone())
    }

    /// Get a session by ID
    #[allow(dead_code)] // Used in tests, reserved for future session lookup
    pub fn get(&self, id: &str) -> Option<FactorySession> {
        self.sessions.get(id).map(|s| s.clone())
    }

    /// Remove expired sessions
    #[allow(dead_code)] // Reserved for background cleanup task
    pub fn cleanup_expired(&self, max_age_secs: u64) {
        let now = Instant::now();
        self.sessions.retain(|_, session| {
            now.duration_since(session.created_at).as_secs() < max_age_secs
        });
    }
}

impl Default for FactorySessions {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if input contains trigger phrases for the factory skill
pub fn check_triggers(input: &str) -> Vec<String> {
    let normalized = input.to_lowercase();
    let mut triggers = Vec::new();

    let trigger_phrases = [
        "teach me",
        "teach you",
        "learn this",
        "learn how",
        "create a skill",
        "remember how to",
        "automate this",
    ];

    for phrase in &trigger_phrases {
        if normalized.contains(phrase) {
            triggers.push(phrase.to_string());
        }
    }

    triggers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_step_prompts() {
        assert_eq!(
            FactoryStep::Goal.get_prompt(),
            "What task do you want me to help with? Give me the high-level goal."
        );
        assert_eq!(
            FactoryStep::Trigger.get_prompt(),
            "When should I use this skill? What words or situations should activate it?"
        );
    }

    #[test]
    fn test_factory_step_next() {
        assert_eq!(FactoryStep::Goal.next(), FactoryStep::Trigger);
        assert_eq!(FactoryStep::Trigger.next(), FactoryStep::Example);
        assert_eq!(FactoryStep::Example.next(), FactoryStep::Complexity);
        assert_eq!(FactoryStep::Complexity.next(), FactoryStep::EdgeCases);
        assert_eq!(FactoryStep::EdgeCases.next(), FactoryStep::Confirm);
        assert_eq!(FactoryStep::Confirm.next(), FactoryStep::Done);
        assert_eq!(FactoryStep::Done.next(), FactoryStep::Done);
    }

    #[test]
    fn test_check_triggers() {
        let triggers = check_triggers("Can you teach me how to do this?");
        assert!(triggers.contains(&"teach me".to_string()));

        let triggers = check_triggers("I want to create a skill for this");
        assert!(triggers.contains(&"create a skill".to_string()));

        let triggers = check_triggers("Please automate this task");
        assert!(triggers.contains(&"automate this".to_string()));

        let triggers = check_triggers("Just a regular message");
        assert!(triggers.is_empty());
    }

    #[test]
    fn test_factory_session_creation() {
        let session = FactorySession::new("test-id".to_string(), None);
        assert_eq!(session.step, FactoryStep::Goal);
        assert!(session.answers.goal.is_none());

        let session = FactorySession::new("test-id".to_string(), Some("Deploy app".to_string()));
        assert_eq!(session.step, FactoryStep::Trigger);
        assert_eq!(session.answers.goal, Some("Deploy app".to_string()));
    }

    #[test]
    fn test_factory_sessions_workflow() {
        let sessions = FactorySessions::new();

        // Start new session
        let session = sessions.start(Some("Deploy my app".to_string()));
        assert_eq!(session.step, FactoryStep::Trigger);

        // Continue with triggers
        let session = sessions.continue_session(&session.id, "deploy, deployment").unwrap();
        assert_eq!(session.step, FactoryStep::Example);
        assert_eq!(session.answers.triggers, Some(vec!["deploy".to_string(), "deployment".to_string()]));

        // Continue with example
        let session = sessions.continue_session(&session.id, "Deploy to production").unwrap();
        assert_eq!(session.step, FactoryStep::Complexity);

        // Continue with complexity
        let session = sessions.continue_session(&session.id, "complex with scripts").unwrap();
        assert_eq!(session.step, FactoryStep::EdgeCases);
        assert_eq!(session.answers.complexity, Some(Complexity::Complex));

        // Continue with edge cases
        let session = sessions.continue_session(&session.id, "Handle missing credentials").unwrap();
        assert_eq!(session.step, FactoryStep::Confirm);

        // Confirm
        let session = sessions.continue_session(&session.id, "yes").unwrap();
        assert_eq!(session.step, FactoryStep::Done);
    }

    #[test]
    fn test_cleanup_expired() {
        let sessions = FactorySessions::new();
        let session = sessions.start(None);

        // Should not cleanup fresh session
        sessions.cleanup_expired(3600);
        assert!(sessions.get(&session.id).is_some());

        // Should cleanup very old sessions (0 seconds = everything is expired)
        sessions.cleanup_expired(0);
        assert!(sessions.get(&session.id).is_none());
    }

    #[test]
    fn test_example_parsing_with_arrow() {
        let sessions = FactorySessions::new();
        let session = sessions.start(Some("Test goal".to_string()));

        // Continue to Example step
        let session = sessions.continue_session(&session.id, "trigger1").unwrap();
        assert_eq!(session.step, FactoryStep::Example);

        // Test arrow separator
        let session = sessions.continue_session(&session.id, "User says 'help me' -> I respond with helpful info").unwrap();
        assert_eq!(session.answers.example_input, Some("User says 'help me'".to_string()));
        assert_eq!(session.answers.example_output, Some("I respond with helpful info".to_string()));
    }

    #[test]
    fn test_example_parsing_with_markers() {
        let sessions = FactorySessions::new();
        let session = sessions.start(Some("Test goal".to_string()));

        // Continue to Example step
        let session = sessions.continue_session(&session.id, "trigger1").unwrap();

        // Test input/output markers
        let session = sessions.continue_session(&session.id, "input: Deploy app output: Success message").unwrap();
        assert_eq!(session.answers.example_input, Some("Deploy app".to_string()));
        assert_eq!(session.answers.example_output, Some("Success message".to_string()));
    }

    #[test]
    fn test_example_parsing_no_separator() {
        let sessions = FactorySessions::new();
        let session = sessions.start(Some("Test goal".to_string()));

        // Continue to Example step
        let session = sessions.continue_session(&session.id, "trigger1").unwrap();

        // Test no separator
        let session = sessions.continue_session(&session.id, "Just an example input").unwrap();
        assert_eq!(session.answers.example_input, Some("Just an example input".to_string()));
        assert_eq!(session.answers.example_output, None);
    }

    #[test]
    fn test_rejection_preserves_answers() {
        let sessions = FactorySessions::new();
        let session = sessions.start(Some("Deploy app".to_string()));

        // Fill in all steps
        let session = sessions.continue_session(&session.id, "deploy").unwrap();
        let session = sessions.continue_session(&session.id, "input -> output").unwrap();
        let session = sessions.continue_session(&session.id, "simple").unwrap();
        let session = sessions.continue_session(&session.id, "Handle errors").unwrap();
        assert_eq!(session.step, FactoryStep::Confirm);

        // Reject and verify answers are preserved
        let session = sessions.continue_session(&session.id, "no").unwrap();
        assert_eq!(session.step, FactoryStep::Goal);
        assert_eq!(session.answers.goal, Some("Deploy app".to_string()));
        assert_eq!(session.answers.triggers, Some(vec!["deploy".to_string()]));
        assert_eq!(session.answers.example_input, Some("input".to_string()));
        assert_eq!(session.answers.example_output, Some("output".to_string()));
        assert_eq!(session.answers.complexity, Some(Complexity::Simple));
        assert_eq!(session.answers.edge_cases, Some("Handle errors".to_string()));
    }
}
