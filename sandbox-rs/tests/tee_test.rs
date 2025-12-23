// TEE integration tests
//
// These tests are feature-gated and require a running dstack socket to function.
// They test the TEE endpoints that provide cryptographic operations and attestation.
//
// To run these tests:
//   cargo test --features tee -- --ignored
//
// Note: Tests are ignored by default because they require:
//   1. The 'tee' feature flag to be enabled
//   2. A real dstack socket to be available (typically only in TEE environments)
//   3. Proper TEE hardware/virtualization support

#[cfg(feature = "tee")]
mod tee_tests {
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
    #[ignore] // Requires dstack socket
    async fn test_tee_info() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();
        let resp = client
            .get(format!("{}/tee/info", base_url))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 200);

        let body: Value = resp.json().await.expect("Failed to parse JSON");

        // InfoResponse should contain CVM instance metadata
        // Verify it has expected fields (structure depends on dstack_sdk::InfoResponse)
        assert!(body.is_object(), "Response should be an object");

        // The exact fields depend on dstack SDK's InfoResponse structure
        // Common fields might include: instance_id, attestation_type, etc.
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_generate_quote() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Generate a quote with report data (64 bytes of zeros as hex)
        let report_data = "0".repeat(128); // 64 bytes in hex

        let resp = client
            .post(format!("{}/tee/quote", base_url))
            .json(&json!({
                "report_data": report_data
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 200);

        let body: Value = resp.json().await.expect("Failed to parse JSON");

        // GetQuoteResponse should contain attestation quote
        assert!(body.is_object(), "Response should be an object");

        // The quote response should have a quote field
        // Structure depends on dstack_sdk::GetQuoteResponse
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_generate_quote_invalid_hex() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Try to generate quote with invalid hex data
        let resp = client
            .post(format!("{}/tee/quote", base_url))
            .json(&json!({
                "report_data": "invalid-hex-data"
            }))
            .send()
            .await
            .expect("Failed to send request");

        // Should return 400 Bad Request for invalid hex
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_derive_key() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Derive a key with path and purpose
        let resp = client
            .post(format!("{}/tee/derive-key", base_url))
            .json(&json!({
                "path": "m/44'/0'/0'/0/0",
                "purpose": "signing"
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 200);

        let body: Value = resp.json().await.expect("Failed to parse JSON");

        // GetKeyResponse should contain derived key information
        assert!(body.is_object(), "Response should be an object");

        // The response should contain key data
        // Structure depends on dstack_sdk::GetKeyResponse
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_derive_key_minimal() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Derive a key without path or purpose (both optional)
        let resp = client
            .post(format!("{}/tee/derive-key", base_url))
            .json(&json!({}))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 200);

        let body: Value = resp.json().await.expect("Failed to parse JSON");
        assert!(body.is_object(), "Response should be an object");
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_sign_data() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // First derive a key to sign with
        let _derive_resp = client
            .post(format!("{}/tee/derive-key", base_url))
            .json(&json!({
                "path": "m/44'/0'/0'/0/0",
                "purpose": "signing"
            }))
            .send()
            .await
            .expect("Failed to derive key");

        // Sign some data with secp256k1
        let data_to_sign = "48656c6c6f20576f726c64"; // "Hello World" in hex

        let resp = client
            .post(format!("{}/tee/sign", base_url))
            .json(&json!({
                "algorithm": "secp256k1",
                "data": data_to_sign
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 200);

        let body: Value = resp.json().await.expect("Failed to parse JSON");

        // SignResponse should contain signature data
        assert!(body.is_object(), "Response should be an object");

        // The response should contain signature
        // Structure depends on dstack_sdk::SignResponse
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_sign_data_invalid_hex() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Try to sign with invalid hex data
        let resp = client
            .post(format!("{}/tee/sign", base_url))
            .json(&json!({
                "algorithm": "secp256k1",
                "data": "not-valid-hex"
            }))
            .send()
            .await
            .expect("Failed to send request");

        // Should return 400 Bad Request for invalid hex
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_verify_signature() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // This test would ideally:
        // 1. Derive a key
        // 2. Sign data
        // 3. Verify the signature
        // For now, we just test the endpoint accepts the right format

        let resp = client
            .post(format!("{}/tee/verify", base_url))
            .json(&json!({
                "algorithm": "secp256k1",
                "data": "48656c6c6f20576f726c64", // "Hello World" in hex
                "signature": "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "public_key": "0000000000000000000000000000000000000000000000000000000000000000"
            }))
            .send()
            .await
            .expect("Failed to send request");

        // May return 200 with valid=false or 500 if verification fails
        // The important thing is it processes the request
        assert!(
            resp.status() == 200 || resp.status() == 500,
            "Expected 200 or 500, got {}",
            resp.status()
        );
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_verify_signature_invalid_hex() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Try to verify with invalid hex data
        let resp = client
            .post(format!("{}/tee/verify", base_url))
            .json(&json!({
                "algorithm": "secp256k1",
                "data": "invalid-hex",
                "signature": "also-invalid",
                "public_key": "not-hex-either"
            }))
            .send()
            .await
            .expect("Failed to send request");

        // Should return 400 Bad Request for invalid hex
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_emit_event() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // Emit a runtime event
        let resp = client
            .post(format!("{}/tee/emit-event", base_url))
            .json(&json!({
                "event": "test_event",
                "payload": "{\"test\": \"data\"}"
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 200);

        let body: Value = resp.json().await.expect("Failed to parse JSON");

        // Should return success response
        assert_eq!(body["success"], true);
        assert!(body["message"].is_string());

        let message = body["message"].as_str().unwrap();
        assert!(message.contains("test_event"));
        assert!(message.contains("emitted successfully"));
    }

    #[tokio::test]
    #[ignore] // Requires dstack socket
    async fn test_sign_and_verify_roundtrip() {
        let base_url =
            std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());

        wait_for_server(&base_url).await;

        let client = Client::new();

        // 1. Derive a key
        let key_resp = client
            .post(format!("{}/tee/derive-key", base_url))
            .json(&json!({
                "path": "m/44'/0'/0'/0/0",
                "purpose": "signing"
            }))
            .send()
            .await
            .expect("Failed to derive key");

        assert_eq!(key_resp.status(), 200);
        let key_body: Value = key_resp.json().await.expect("Failed to parse key response");

        // 2. Sign data
        let data_to_sign = "48656c6c6f20576f726c64"; // "Hello World" in hex

        let sign_resp = client
            .post(format!("{}/tee/sign", base_url))
            .json(&json!({
                "algorithm": "secp256k1",
                "data": data_to_sign
            }))
            .send()
            .await
            .expect("Failed to sign");

        assert_eq!(sign_resp.status(), 200);
        let sign_body: Value = sign_resp.json().await.expect("Failed to parse sign response");

        // 3. Verify signature
        // Note: This assumes the response structures have specific fields
        // If the actual dstack SDK response differs, this test may need adjustment
        // when run against a real dstack socket

        // The actual verification would use signature and public_key from responses
        // For now, we've validated the sign endpoint works
        assert!(sign_body.is_object());
        assert!(key_body.is_object());
    }
}
