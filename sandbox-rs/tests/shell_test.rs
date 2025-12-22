use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

async fn wait_for_server(base_url: &str) {
    let client = Client::new();
    for _ in 0..50 {
        if client.get(format!("{}/health", base_url)).send().await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server did not start in time");
}

#[tokio::test]
async fn test_shell_exec_simple() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "echo hello"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["stdout"].as_str().unwrap().trim(), "hello");
    assert_eq!(body["exit_code"], 0);
}

#[tokio::test]
async fn test_shell_exec_with_env() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "echo $MY_VAR",
            "env": { "MY_VAR": "test_value" }
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["stdout"].as_str().unwrap().trim(), "test_value");
}

#[tokio::test]
async fn test_shell_exec_stderr() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "echo error >&2"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["stderr"].as_str().unwrap().trim(), "error");
}

#[tokio::test]
async fn test_shell_exec_nonzero_exit() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/shell/exec", base_url))
        .json(&json!({
            "command": "exit 42"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["exit_code"], 42);
}
