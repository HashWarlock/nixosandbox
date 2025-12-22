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
async fn test_file_write_and_read() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Write file
    let write_resp = client
        .post(format!("{}/file/write", base_url))
        .json(&json!({
            "path": "/tmp/test_file.txt",
            "content": "hello world"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(write_resp.status(), 200);

    // Read file back
    let read_resp = client
        .get(format!("{}/file/read?path=/tmp/test_file.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(read_resp.status(), 200);

    let body: Value = read_resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["content"], "hello world");
}

#[tokio::test]
async fn test_file_list() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // Ensure /tmp exists and list it
    let resp = client
        .get(format!("{}/file/list?path=/tmp", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("Failed to parse JSON");
    assert!(body["entries"].is_array());
}

#[tokio::test]
async fn test_file_not_found() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/file/read?path=/nonexistent/file.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_file_download() {
    let base_url = std::env::var("TEST_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    wait_for_server(&base_url).await;

    let client = Client::new();

    // First write a file
    client
        .post(format!("{}/file/write", base_url))
        .json(&json!({
            "path": "/tmp/download_test.txt",
            "content": "download content"
        }))
        .send()
        .await
        .expect("Failed to write file");

    // Download it
    let resp = client
        .get(format!("{}/file/download?path=/tmp/download_test.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("content-disposition").is_some());

    let content = resp.text().await.expect("Failed to get body");
    assert_eq!(content, "download content");
}
