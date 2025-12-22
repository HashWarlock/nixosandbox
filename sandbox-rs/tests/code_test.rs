use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
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

#[tokio::test]
async fn test_code_python() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/code/execute", base_url))
        .json(&json!({
            "code": "print('hello from python')",
            "language": "python"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["output"].as_str().unwrap().trim(), "hello from python");
    assert_eq!(body["exit_code"], 0);
}

#[tokio::test]
async fn test_code_bash() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/code/execute", base_url))
        .json(&json!({
            "code": "echo 'hello from bash'",
            "language": "bash"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["output"].as_str().unwrap().trim(), "hello from bash");
}

#[tokio::test]
async fn test_code_unsupported_language() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/code/execute", base_url))
        .json(&json!({
            "code": "print('hello')",
            "language": "cobol"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 400);
}
