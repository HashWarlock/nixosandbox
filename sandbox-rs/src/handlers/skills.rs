use axum::{
    extract::{Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::process::Command;

use crate::error::{AppError, Result};
use crate::skills::{CreateSkillRequest, Skill, SkillRegistry, SkillSummary, UpdateSkillRequest};

// GET /skills - List all skills
#[derive(Serialize)]
pub struct ListSkillsResponse {
    pub skills: Vec<SkillSummary>,
}

pub async fn list_skills(registry: &SkillRegistry) -> Result<Json<ListSkillsResponse>> {
    let skills = registry.list().await?;
    Ok(Json(ListSkillsResponse { skills }))
}

// GET /skills/search - Search skills by query
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search_skills(
    registry: &SkillRegistry,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ListSkillsResponse>> {
    let skills = registry.search(&query.q).await?;
    Ok(Json(ListSkillsResponse { skills }))
}

// GET /skills/:name - Get a specific skill
pub async fn get_skill(
    registry: &SkillRegistry,
    Path(name): Path<String>,
) -> Result<Json<Skill>> {
    let skill = registry.get(&name).await?;
    Ok(Json(skill))
}

// POST /skills - Create a new skill
#[derive(Deserialize)]
pub struct CreateSkillRequestJson {
    pub name: String,
    pub description: String,
    pub body: String,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub references: HashMap<String, String>,
    #[serde(default)]
    pub assets: HashMap<String, String>,
}

pub async fn create_skill(
    registry: &SkillRegistry,
    Json(req): Json<CreateSkillRequestJson>,
) -> Result<Json<Skill>> {
    let create_req = CreateSkillRequest {
        name: req.name,
        description: req.description,
        body: req.body,
        scripts: req.scripts,
        references: req.references,
        assets: req.assets,
    };

    let skill = registry.create(create_req).await?;
    Ok(Json(skill))
}

// PUT /skills/:name - Update an existing skill
#[derive(Deserialize)]
pub struct UpdateSkillRequestJson {
    pub description: Option<String>,
    pub body: Option<String>,
    pub scripts: Option<HashMap<String, String>>,
    pub references: Option<HashMap<String, String>>,
    pub assets: Option<HashMap<String, String>>,
}

pub async fn update_skill(
    registry: &SkillRegistry,
    Path(name): Path<String>,
    Json(req): Json<UpdateSkillRequestJson>,
) -> Result<Json<Skill>> {
    let update_req = UpdateSkillRequest {
        description: req.description,
        body: req.body,
        scripts: req.scripts,
        references: req.references,
        assets: req.assets,
    };

    let skill = registry.update(&name, update_req).await?;
    Ok(Json(skill))
}

// DELETE /skills/:name - Delete a skill
#[derive(Serialize)]
pub struct DeleteSkillResponse {
    pub success: bool,
    pub message: String,
}

pub async fn delete_skill(
    registry: &SkillRegistry,
    Path(name): Path<String>,
) -> Result<Json<DeleteSkillResponse>> {
    registry.delete(&name).await?;
    Ok(Json(DeleteSkillResponse {
        success: true,
        message: format!("Skill '{}' deleted successfully", name),
    }))
}

// POST /skills/:name/scripts/:script - Execute a script
#[derive(Deserialize)]
pub struct ExecuteScriptRequest {
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ExecuteScriptResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn execute_script(
    registry: &SkillRegistry,
    Path((skill_name, script_name)): Path<(String, String)>,
    Json(req): Json<ExecuteScriptRequest>,
) -> Result<Json<ExecuteScriptResponse>> {
    // Get the skill to verify it exists
    let skill = registry.get(&skill_name).await?;

    // Verify the script exists
    if !skill.scripts.contains(&script_name) {
        return Err(AppError::NotFound(format!(
            "Script '{}' not found in skill '{}'",
            script_name, skill_name
        )));
    }

    // Build the script path using the registry's internal path
    // The registry knows where skills are stored
    let skill_dir = registry.skill_dir(&skill_name);
    let scripts_dir = skill_dir.join("scripts");
    let script_path = scripts_dir.join(&script_name);

    if !script_path.exists() {
        return Err(AppError::NotFound(format!(
            "Script file not found: {}",
            script_path.display()
        )));
    }

    // Determine how to execute the script based on its extension
    let (command, args) = if script_name.ends_with(".sh") {
        ("sh", vec![script_path.to_string_lossy().to_string()])
    } else if script_name.ends_with(".py") {
        ("python3", vec![script_path.to_string_lossy().to_string()])
    } else if script_name.ends_with(".js") {
        ("node", vec![script_path.to_string_lossy().to_string()])
    } else {
        // Default: try to execute directly
        (script_path.to_str().unwrap(), vec![])
    };

    // Build the command with user-provided args
    let mut cmd = Command::new(command);
    cmd.current_dir(&scripts_dir);

    // Add script path and user args
    for arg in args {
        cmd.arg(arg);
    }
    for arg in &req.args {
        cmd.arg(arg);
    }

    // Add environment variables
    for (key, value) in &req.env {
        cmd.env(key, value);
    }

    // Execute the command
    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute script: {}", e)))?;

    Ok(Json(ExecuteScriptResponse {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    }))
}
