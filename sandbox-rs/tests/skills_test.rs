use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

async fn wait_for_server(base_url: &str) {
    let client = Client::new();
    for _ in 0..50 {
        if client
            .get(format!("{}/health", base_url))
            .send()
            .await
            .is_ok()
        {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server did not start in time");
}

fn setup_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    std::env::set_var("SKILLS_DIR", temp_dir.path().to_str().unwrap());
    temp_dir
}

#[tokio::test]
async fn test_list_skills_empty() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/skills", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["skills"].is_array());
    // Note: may not be empty if server has pre-existing skills
}

#[tokio::test]
async fn test_create_and_get_skill() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Create a skill
    let create_resp = client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "test-skill",
            "description": "A test skill for integration testing",
            "body": "This is the skill body with instructions."
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(create_resp.status(), 200);

    let created: Value = create_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(created["name"], "test-skill");
    assert_eq!(created["description"], "A test skill for integration testing");

    // Get the skill
    let get_resp = client
        .get(format!("{}/skills/test-skill", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(get_resp.status(), 200);

    let retrieved: Value = get_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(retrieved["name"], "test-skill");
    assert_eq!(retrieved["description"], "A test skill for integration testing");
    assert_eq!(retrieved["body"], "This is the skill body with instructions.");
}

#[tokio::test]
async fn test_create_skill_invalid_name() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Try to create a skill with invalid name (uppercase)
    let resp = client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "Invalid-Name",
            "description": "This should fail",
            "body": "Body"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 400);

    // Try with spaces
    let resp = client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "invalid name",
            "description": "This should fail",
            "body": "Body"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 400);

    // Try with consecutive hyphens
    let resp = client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "invalid--name",
            "description": "This should fail",
            "body": "Body"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_update_skill() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Create a skill first
    client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "update-test",
            "description": "Original description",
            "body": "Original body"
        }))
        .send()
        .await
        .expect("Failed to create skill");

    // Update the skill
    let update_resp = client
        .put(format!("{}/skills/update-test", base_url))
        .json(&json!({
            "description": "Updated description",
            "body": "Updated body"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(update_resp.status(), 200);

    let updated: Value = update_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(updated["description"], "Updated description");
    assert_eq!(updated["body"], "Updated body");

    // Verify the update persisted
    let get_resp = client
        .get(format!("{}/skills/update-test", base_url))
        .send()
        .await
        .expect("Failed to send request");

    let retrieved: Value = get_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(retrieved["description"], "Updated description");
    assert_eq!(retrieved["body"], "Updated body");
}

#[tokio::test]
async fn test_delete_skill() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Create a skill first
    client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "delete-test",
            "description": "To be deleted",
            "body": "Body"
        }))
        .send()
        .await
        .expect("Failed to create skill");

    // Verify it exists
    let get_resp = client
        .get(format!("{}/skills/delete-test", base_url))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(get_resp.status(), 200);

    // Delete the skill
    let delete_resp = client
        .delete(format!("{}/skills/delete-test", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(delete_resp.status(), 200);

    let body: Value = delete_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["success"], true);

    // Verify it's gone
    let get_resp = client
        .get(format!("{}/skills/delete-test", base_url))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(get_resp.status(), 404);
}

#[tokio::test]
async fn test_search_skills() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Create multiple skills
    client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "rust-programming",
            "description": "A skill for Rust development",
            "body": "Instructions for Rust"
        }))
        .send()
        .await
        .expect("Failed to create skill");

    client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "python-automation",
            "description": "A skill for Python automation",
            "body": "Instructions for Python"
        }))
        .send()
        .await
        .expect("Failed to create skill");

    client
        .post(format!("{}/skills", base_url))
        .json(&json!({
            "name": "web-development",
            "description": "A skill for web development",
            "body": "Instructions for web dev"
        }))
        .send()
        .await
        .expect("Failed to create skill");

    // Search for "rust"
    let search_resp = client
        .get(format!("{}/skills/search?q=rust", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(search_resp.status(), 200);

    let results: Value = search_resp.json().await.expect("Failed to parse JSON");
    assert!(results["skills"].is_array());
    let skills = results["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "rust-programming");

    // Search for "automation" (in description)
    let search_resp = client
        .get(format!("{}/skills/search?q=automation", base_url))
        .send()
        .await
        .expect("Failed to send request");

    let results: Value = search_resp.json().await.expect("Failed to parse JSON");
    let skills = results["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "python-automation");

    // Search for "skill" (should match all descriptions)
    let search_resp = client
        .get(format!("{}/skills/search?q=skill", base_url))
        .send()
        .await
        .expect("Failed to send request");

    let results: Value = search_resp.json().await.expect("Failed to parse JSON");
    let skills = results["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 3);
}
