use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use crate::error::{AppError, Result};
use super::types::{Skill, SkillMeta, SkillSummary, validate_skill_name, validate_description};

/// Request to create a new skill
#[derive(Debug, Clone)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub body: String,
    pub scripts: HashMap<String, String>,      // filename -> content
    pub references: HashMap<String, String>,
    pub assets: HashMap<String, String>,
}

/// Request to update an existing skill
#[derive(Debug, Clone, Default)]
pub struct UpdateSkillRequest {
    pub description: Option<String>,
    pub body: Option<String>,
    pub scripts: Option<HashMap<String, String>>,
    pub references: Option<HashMap<String, String>>,
    pub assets: Option<HashMap<String, String>>,
}

/// Registry for managing skills in the filesystem
pub struct SkillRegistry {
    skills_dir: PathBuf,
}

/// Validate that a filename doesn't contain path traversal sequences
fn validate_filename(filename: &str) -> Result<()> {
    if filename.is_empty() {
        return Err(AppError::BadRequest("Filename cannot be empty".into()));
    }
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(AppError::BadRequest("Invalid filename: path traversal not allowed".into()));
    }
    Ok(())
}

impl SkillRegistry {
    /// Create a new skill registry
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    /// Ensure the skills directory exists
    async fn ensure_skills_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.skills_dir).await?;
        Ok(())
    }

    /// Get the path to a skill directory
    fn skill_path(&self, name: &str) -> PathBuf {
        self.skills_dir.join(name)
    }

    /// Get the path to a skill directory (public accessor)
    pub fn skill_dir(&self, name: &str) -> PathBuf {
        self.skill_path(name)
    }

    /// Get the path to a skill's SKILL.md file
    fn skill_md_path(&self, name: &str) -> PathBuf {
        self.skill_path(name).join("SKILL.md")
    }

    /// List all skills
    pub async fn list(&self) -> Result<Vec<SkillSummary>> {
        self.ensure_skills_dir().await?;

        let mut entries = fs::read_dir(&self.skills_dir).await?;
        let mut summaries = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Try to read the skill to get its description
            match self.get(&name).await {
                Ok(skill) => {
                    summaries.push(SkillSummary {
                        name: skill.meta.name,
                        description: skill.meta.description,
                    });
                }
                Err(_) => {
                    // Skip invalid skills
                    continue;
                }
            }
        }

        summaries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(summaries)
    }

    /// Get a skill by name
    pub async fn get(&self, name: &str) -> Result<Skill> {
        validate_skill_name(name).map_err(|e| AppError::BadRequest(e))?;

        let skill_md_path = self.skill_md_path(name);
        if !skill_md_path.exists() {
            return Err(AppError::NotFound(format!("Skill '{}' not found", name)));
        }

        let content = fs::read_to_string(&skill_md_path).await?;
        let (meta, body) = self.parse_skill_md(&content)?;

        // List scripts, references, and assets
        let skill_dir = self.skill_path(name);
        let scripts = self.list_dir_files(&skill_dir.join("scripts")).await?;
        let references = self.list_dir_files(&skill_dir.join("references")).await?;
        let assets = self.list_dir_files(&skill_dir.join("assets")).await?;

        Ok(Skill {
            meta,
            body,
            scripts,
            references,
            assets,
        })
    }

    /// Create a new skill
    pub async fn create(&self, req: CreateSkillRequest) -> Result<Skill> {
        validate_skill_name(&req.name).map_err(|e| AppError::BadRequest(e))?;
        validate_description(&req.description).map_err(|e| AppError::BadRequest(e))?;

        let skill_dir = self.skill_path(&req.name);
        if skill_dir.exists() {
            return Err(AppError::BadRequest(format!("Skill '{}' already exists", req.name)));
        }

        // Create skill directory structure
        fs::create_dir_all(&skill_dir).await?;
        fs::create_dir_all(skill_dir.join("scripts")).await?;
        fs::create_dir_all(skill_dir.join("references")).await?;
        fs::create_dir_all(skill_dir.join("assets")).await?;

        // Create metadata
        let meta = SkillMeta {
            name: req.name.clone(),
            description: req.description.clone(),
            license: None,
            compatibility: None,
            metadata: None,
        };

        // Write SKILL.md
        let skill_md = self.format_skill_md(&meta, &req.body);
        fs::write(self.skill_md_path(&req.name), skill_md).await?;

        // Write scripts
        for (filename, content) in &req.scripts {
            validate_filename(filename)?;
            let script_path = skill_dir.join("scripts").join(filename);
            fs::write(script_path, content).await?;
        }

        // Write references
        for (filename, content) in &req.references {
            validate_filename(filename)?;
            let ref_path = skill_dir.join("references").join(filename);
            fs::write(ref_path, content).await?;
        }

        // Write assets
        for (filename, content) in &req.assets {
            validate_filename(filename)?;
            let asset_path = skill_dir.join("assets").join(filename);
            fs::write(asset_path, content).await?;
        }

        self.get(&req.name).await
    }

    /// Update an existing skill
    pub async fn update(&self, name: &str, req: UpdateSkillRequest) -> Result<Skill> {
        validate_skill_name(name).map_err(|e| AppError::BadRequest(e))?;

        // Get existing skill
        let mut skill = self.get(name).await?;
        let skill_dir = self.skill_path(name);

        // Update metadata if description changed
        if let Some(description) = &req.description {
            validate_description(description).map_err(|e| AppError::BadRequest(e))?;
            skill.meta.description = description.clone();
        }

        // Update body if provided
        if let Some(body) = &req.body {
            skill.body = body.clone();
        }

        // Write updated SKILL.md
        let skill_md = self.format_skill_md(&skill.meta, &skill.body);
        fs::write(self.skill_md_path(name), skill_md).await?;

        // Update scripts if provided
        if let Some(scripts) = &req.scripts {
            let scripts_dir = skill_dir.join("scripts");
            // Remove old scripts
            if scripts_dir.exists() {
                fs::remove_dir_all(&scripts_dir).await?;
            }
            fs::create_dir_all(&scripts_dir).await?;
            // Write new scripts
            for (filename, content) in scripts {
                validate_filename(filename)?;
                let script_path = scripts_dir.join(filename);
                fs::write(script_path, content).await?;
            }
        }

        // Update references if provided
        if let Some(references) = &req.references {
            let references_dir = skill_dir.join("references");
            // Remove old references
            if references_dir.exists() {
                fs::remove_dir_all(&references_dir).await?;
            }
            fs::create_dir_all(&references_dir).await?;
            // Write new references
            for (filename, content) in references {
                validate_filename(filename)?;
                let ref_path = references_dir.join(filename);
                fs::write(ref_path, content).await?;
            }
        }

        // Update assets if provided
        if let Some(assets) = &req.assets {
            let assets_dir = skill_dir.join("assets");
            // Remove old assets
            if assets_dir.exists() {
                fs::remove_dir_all(&assets_dir).await?;
            }
            fs::create_dir_all(&assets_dir).await?;
            // Write new assets
            for (filename, content) in assets {
                validate_filename(filename)?;
                let asset_path = assets_dir.join(filename);
                fs::write(asset_path, content).await?;
            }
        }

        self.get(name).await
    }

    /// Delete a skill
    pub async fn delete(&self, name: &str) -> Result<()> {
        validate_skill_name(name).map_err(|e| AppError::BadRequest(e))?;

        let skill_dir = self.skill_path(name);
        if !skill_dir.exists() {
            return Err(AppError::NotFound(format!("Skill '{}' not found", name)));
        }

        fs::remove_dir_all(&skill_dir).await?;
        Ok(())
    }

    /// Search for skills by query (searches name and description)
    pub async fn search(&self, query: &str) -> Result<Vec<SkillSummary>> {
        let all_skills = self.list().await?;
        let query_lower = query.to_lowercase();

        let results: Vec<SkillSummary> = all_skills
            .into_iter()
            .filter(|skill| {
                skill.name.to_lowercase().contains(&query_lower)
                    || skill.description.to_lowercase().contains(&query_lower)
            })
            .collect();

        Ok(results)
    }

    /// Parse SKILL.md into metadata and body
    fn parse_skill_md(&self, content: &str) -> Result<(SkillMeta, String)> {
        // Split on --- to extract frontmatter
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(AppError::BadRequest(
                "Invalid SKILL.md format: missing frontmatter delimiters".to_string(),
            ));
        }

        // Parse YAML frontmatter (parts[1])
        let frontmatter = parts[1].trim();
        let meta: SkillMeta = serde_yaml::from_str(frontmatter)
            .map_err(|e| AppError::BadRequest(format!("Failed to parse frontmatter: {}", e)))?;

        // Body is everything after the second ---
        let body = parts[2].trim().to_string();

        Ok((meta, body))
    }

    /// Format SKILL.md from metadata and body
    fn format_skill_md(&self, meta: &SkillMeta, body: &str) -> String {
        let frontmatter = serde_yaml::to_string(meta).unwrap_or_default();
        format!("---\n{}---\n\n{}", frontmatter, body)
    }

    /// List files in a directory
    async fn list_dir_files(&self, dir: &PathBuf) -> Result<Vec<String>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(dir).await?;
        let mut files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.push(name.to_string());
                }
            }
        }

        files.sort();
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_registry() -> (SkillRegistry, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let registry = SkillRegistry::new(temp_dir.path().to_path_buf());
        (registry, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_get_skill() {
        let (registry, _temp) = create_test_registry().await;

        let req = CreateSkillRequest {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            body: "This is the skill body".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        let created = registry.create(req).await.unwrap();
        assert_eq!(created.meta.name, "test-skill");
        assert_eq!(created.meta.description, "A test skill");
        assert_eq!(created.body, "This is the skill body");

        let retrieved = registry.get("test-skill").await.unwrap();
        assert_eq!(retrieved.meta.name, "test-skill");
    }

    #[tokio::test]
    async fn test_list_skills() {
        let (registry, _temp) = create_test_registry().await;

        let req1 = CreateSkillRequest {
            name: "skill-one".to_string(),
            description: "First skill".to_string(),
            body: "Body 1".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        let req2 = CreateSkillRequest {
            name: "skill-two".to_string(),
            description: "Second skill".to_string(),
            body: "Body 2".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        registry.create(req1).await.unwrap();
        registry.create(req2).await.unwrap();

        let skills = registry.list().await.unwrap();
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "skill-one");
        assert_eq!(skills[1].name, "skill-two");
    }

    #[tokio::test]
    async fn test_update_skill() {
        let (registry, _temp) = create_test_registry().await;

        let req = CreateSkillRequest {
            name: "update-test".to_string(),
            description: "Original description".to_string(),
            body: "Original body".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        registry.create(req).await.unwrap();

        let update_req = UpdateSkillRequest {
            description: Some("Updated description".to_string()),
            body: Some("Updated body".to_string()),
            ..Default::default()
        };

        let updated = registry.update("update-test", update_req).await.unwrap();
        assert_eq!(updated.meta.description, "Updated description");
        assert_eq!(updated.body, "Updated body");
    }

    #[tokio::test]
    async fn test_delete_skill() {
        let (registry, _temp) = create_test_registry().await;

        let req = CreateSkillRequest {
            name: "delete-me".to_string(),
            description: "To be deleted".to_string(),
            body: "Body".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        registry.create(req).await.unwrap();
        assert!(registry.get("delete-me").await.is_ok());

        registry.delete("delete-me").await.unwrap();
        assert!(registry.get("delete-me").await.is_err());
    }

    #[tokio::test]
    async fn test_search_skills() {
        let (registry, _temp) = create_test_registry().await;

        let req1 = CreateSkillRequest {
            name: "rust-skill".to_string(),
            description: "A skill for Rust programming".to_string(),
            body: "Body".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        let req2 = CreateSkillRequest {
            name: "python-skill".to_string(),
            description: "A skill for Python programming".to_string(),
            body: "Body".to_string(),
            scripts: HashMap::new(),
            references: HashMap::new(),
            assets: HashMap::new(),
        };

        registry.create(req1).await.unwrap();
        registry.create(req2).await.unwrap();

        let results = registry.search("rust").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rust-skill");

        let results = registry.search("programming").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_parse_skill_md() {
        let (registry, _temp) = create_test_registry().await;

        let content = r#"---
name: test-skill
description: A test skill
---

This is the body content
with multiple lines
"#;

        let (meta, body) = registry.parse_skill_md(content).unwrap();
        assert_eq!(meta.name, "test-skill");
        assert_eq!(meta.description, "A test skill");
        assert!(body.contains("This is the body content"));
    }
}
