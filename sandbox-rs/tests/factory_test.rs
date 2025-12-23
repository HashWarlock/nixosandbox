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
async fn test_factory_full_flow() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Step 1: Start factory session with initial goal
    let start_resp = client
        .post(format!("{}/factory/start", base_url))
        .json(&json!({
            "initial_input": "Deploy my application to production"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(start_resp.status(), 200);

    let start_body: Value = start_resp.json().await.expect("Failed to parse JSON");
    let session_id = start_body["session_id"].as_str().unwrap().to_string();
    assert_eq!(start_body["step"], "Trigger");
    assert_eq!(start_body["done"], false);
    assert!(start_body["prompt"].as_str().unwrap().contains("When should I use this skill"));

    // Step 2: Provide triggers
    let continue_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "deploy, deployment, ship to production"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(continue_resp.status(), 200);

    let body: Value = continue_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "Example");
    assert_eq!(body["done"], false);
    assert!(body["prompt"].as_str().unwrap().contains("Walk me through a real example"));

    // Step 3: Provide example
    let continue_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "input: Deploy v2.0 to production -> output: Application deployed successfully"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(continue_resp.status(), 200);

    let body: Value = continue_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "Complexity");
    assert_eq!(body["done"], false);
    assert!(body["prompt"].as_str().unwrap().contains("simple skill"));

    // Step 4: Provide complexity
    let continue_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "complex - needs scripts for deployment"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(continue_resp.status(), 200);

    let body: Value = continue_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "EdgeCases");
    assert_eq!(body["done"], false);
    assert!(body["prompt"].as_str().unwrap().contains("missing or goes wrong"));

    // Step 5: Provide edge cases
    let continue_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "Handle connection errors and rollback on failure"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(continue_resp.status(), 200);

    let body: Value = continue_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "Confirm");
    assert_eq!(body["done"], false);
    assert!(body["prompt"].as_str().unwrap().contains("Does this capture what you want"));
    // Should include summary in the prompt
    assert!(body["prompt"].as_str().unwrap().contains("Skill Summary"));

    // Step 6: Confirm
    let continue_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "yes"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(continue_resp.status(), 200);

    let body: Value = continue_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "Done");
    assert_eq!(body["done"], true);
    assert!(body["skill"].is_object());

    let skill = &body["skill"];
    assert!(skill["name"].as_str().unwrap().contains("deploy"));
    assert!(skill["description"].as_str().is_some());
}

#[tokio::test]
async fn test_factory_session_not_found() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Try to continue with a non-existent session ID
    let resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": "non-existent-session-id",
            "input": "some input"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_check_trigger_phrases() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Test input that should trigger
    let resp = client
        .post(format!("{}/factory/check", base_url))
        .json(&json!({
            "input": "Can you teach me how to do this task?"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["triggers_factory"], true);
    assert!(body["matched_phrases"].is_array());
    let phrases = body["matched_phrases"].as_array().unwrap();
    assert!(phrases.len() > 0);
    assert!(phrases.iter().any(|p| p.as_str().unwrap() == "teach me"));

    // Test "create a skill" trigger
    let resp = client
        .post(format!("{}/factory/check", base_url))
        .json(&json!({
            "input": "I want to create a skill for this"
        }))
        .send()
        .await
        .expect("Failed to send request");

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["triggers_factory"], true);
    let phrases = body["matched_phrases"].as_array().unwrap();
    assert!(phrases.iter().any(|p| p.as_str().unwrap() == "create a skill"));

    // Test "automate this" trigger
    let resp = client
        .post(format!("{}/factory/check", base_url))
        .json(&json!({
            "input": "Please help me automate this process"
        }))
        .send()
        .await
        .expect("Failed to send request");

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["triggers_factory"], true);
    let phrases = body["matched_phrases"].as_array().unwrap();
    assert!(phrases.iter().any(|p| p.as_str().unwrap() == "automate this"));

    // Test input that should NOT trigger
    let resp = client
        .post(format!("{}/factory/check", base_url))
        .json(&json!({
            "input": "What's the weather today?"
        }))
        .send()
        .await
        .expect("Failed to send request");

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["triggers_factory"], false);
    let phrases = body["matched_phrases"].as_array().unwrap();
    assert_eq!(phrases.len(), 0);
}

#[tokio::test]
async fn test_factory_start_without_initial_input() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Start factory session without initial input
    let start_resp = client
        .post(format!("{}/factory/start", base_url))
        .json(&json!({}))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(start_resp.status(), 200);

    let start_body: Value = start_resp.json().await.expect("Failed to parse JSON");
    let session_id = start_body["session_id"].as_str().unwrap().to_string();
    assert_eq!(start_body["step"], "Goal");
    assert_eq!(start_body["done"], false);
    assert!(start_body["prompt"].as_str().unwrap().contains("What task do you want me to help with"));

    // Now provide the goal
    let continue_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "Create PDF reports"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(continue_resp.status(), 200);

    let body: Value = continue_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "Trigger");
}

#[tokio::test]
async fn test_factory_rejection_and_restart() {
    let _temp = setup_test_env();
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Start and go through steps
    let start_resp = client
        .post(format!("{}/factory/start", base_url))
        .json(&json!({
            "initial_input": "Test goal"
        }))
        .send()
        .await
        .expect("Failed to send request");

    let start_body: Value = start_resp.json().await.expect("Failed to parse JSON");
    let session_id = start_body["session_id"].as_str().unwrap().to_string();

    // Go through all steps quickly
    client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "trigger1, trigger2"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "example input"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "simple"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "edge cases"
        }))
        .send()
        .await
        .unwrap();

    // Reject at confirmation
    let reject_resp = client
        .post(format!("{}/factory/continue", base_url))
        .json(&json!({
            "session_id": session_id,
            "input": "no"
        }))
        .send()
        .await
        .expect("Failed to send request");

    let body: Value = reject_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["step"], "Goal");
    assert_eq!(body["done"], false);
}
