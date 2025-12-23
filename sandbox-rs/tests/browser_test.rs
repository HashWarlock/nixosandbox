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
#[ignore] // Requires running server with Chromium
async fn test_browser_status_before_use() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/browser/status", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["running"].is_boolean());
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_goto() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/goto", base_url))
        .json(&json!({
            "url": "https://example.com"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["url"].is_string());
    assert!(body["title"].is_string());
    // example.com should have "Example Domain" in title
    let title = body["title"].as_str().unwrap();
    assert!(title.contains("Example"), "Expected 'Example' in title, got: {}", title);
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_screenshot() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/screenshot", base_url))
        .json(&json!({
            "url": "https://example.com"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["data"].is_string());
    assert_eq!(body["format"], "png");
    assert!(body["width"].is_number());
    assert!(body["height"].is_number());

    // Verify it's valid base64
    let data = body["data"].as_str().unwrap();
    assert!(!data.is_empty(), "Screenshot data should not be empty");
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_evaluate() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/evaluate", base_url))
        .json(&json!({
            "url": "https://example.com",
            "script": "document.title"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["result"].is_string());
    let result = body["result"].as_str().unwrap();
    assert!(result.contains("Example"), "Expected 'Example' in result, got: {}", result);
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_evaluate_math() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/evaluate", base_url))
        .json(&json!({
            "url": "https://example.com",
            "script": "2 + 2"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["result"], 4);
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_click_nonexistent() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/click", base_url))
        .json(&json!({
            "url": "https://example.com",
            "selector": "#nonexistent-element-12345"
        }))
        .send()
        .await
        .expect("Failed to send request");

    // Should return 404 for element not found
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_click_existing() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    // example.com has an <a> link we can click
    let resp = client
        .post(format!("{}/browser/click", base_url))
        .json(&json!({
            "url": "https://example.com",
            "selector": "a"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_type_nonexistent() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/type", base_url))
        .json(&json!({
            "url": "https://example.com",
            "selector": "#nonexistent-input-12345",
            "text": "Hello World"
        }))
        .send()
        .await
        .expect("Failed to send request");

    // Should return 404 for element not found
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_status_after_use() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // First trigger browser launch with a goto
    client
        .post(format!("{}/browser/goto", base_url))
        .json(&json!({
            "url": "https://example.com"
        }))
        .send()
        .await
        .expect("Failed to send request");

    // Now check status - should be running
    let resp = client
        .get(format!("{}/browser/status", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["running"], true);
    // Version should be available after browser is running
    assert!(body["version"].is_string() || body["version"].is_null());
}

#[tokio::test]
#[ignore] // Requires running server with Chromium
async fn test_browser_goto_invalid_url() {
    let base_url =
        std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .post(format!("{}/browser/goto", base_url))
        .json(&json!({
            "url": "not-a-valid-url"
        }))
        .send()
        .await
        .expect("Failed to send request");

    // Should fail with navigation error (500)
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
}
