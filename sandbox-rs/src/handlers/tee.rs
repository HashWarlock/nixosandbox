#[cfg(feature = "tee")]
use axum::{extract::State, Json};
use dstack_sdk::dstack_client::{
    GetKeyResponse, GetQuoteResponse, InfoResponse, SignResponse, VerifyResponse,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::state::AppState;

// Request types
#[derive(Deserialize)]
pub struct GenerateQuoteRequest {
    pub report_data: String, // hex-encoded
}

#[derive(Deserialize)]
pub struct DeriveKeyRequest {
    pub path: Option<String>,
    pub purpose: Option<String>,
}

#[derive(Deserialize)]
pub struct SignRequest {
    pub algorithm: String, // "secp256k1"
    pub data: String,      // hex-encoded
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub algorithm: String,
    pub data: String,
    pub signature: String,
    pub public_key: String,
}

#[derive(Deserialize)]
pub struct EmitEventRequest {
    pub event: String,
    pub payload: String,
}

// Helper function to decode hex strings
fn decode_hex(s: &str) -> Result<Vec<u8>> {
    hex::decode(s).map_err(|e| AppError::BadRequest(format!("Invalid hex string: {}", e)))
}

// GET /tee/info - CVM instance metadata
pub async fn tee_info(State(state): State<Arc<AppState>>) -> Result<Json<InfoResponse>> {
    let info = state
        .tee_service
        .info()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get TEE info: {}", e)))?;

    Ok(Json(info))
}

// POST /tee/quote - TDX attestation quote
pub async fn generate_quote(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GenerateQuoteRequest>,
) -> Result<Json<GetQuoteResponse>> {
    let report_data = decode_hex(&req.report_data)?;

    let quote = state
        .tee_service
        .get_quote(&report_data)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to generate quote: {}", e)))?;

    Ok(Json(quote))
}

// POST /tee/derive-key - Derive key with path/purpose
pub async fn derive_key(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeriveKeyRequest>,
) -> Result<Json<GetKeyResponse>> {
    let key = state
        .tee_service
        .derive_key(req.path.as_deref(), req.purpose.as_deref())
        .await
        .map_err(|e| AppError::Internal(format!("Failed to derive key: {}", e)))?;

    Ok(Json(key))
}

// POST /tee/sign - Sign with derived key
pub async fn sign_data(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignRequest>,
) -> Result<Json<SignResponse>> {
    let data = decode_hex(&req.data)?;

    let signature = state
        .tee_service
        .sign(&req.algorithm, &data)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to sign data: {}", e)))?;

    Ok(Json(signature))
}

// POST /tee/verify - Verify signature
pub async fn verify_signature(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>> {
    let data = decode_hex(&req.data)?;
    let signature = decode_hex(&req.signature)?;
    let public_key = decode_hex(&req.public_key)?;

    let result = state
        .tee_service
        .verify(&req.algorithm, &data, &signature, &public_key)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to verify signature: {}", e)))?;

    Ok(Json(result))
}

// POST /tee/emit-event - Emit runtime event
pub async fn emit_event(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmitEventRequest>,
) -> Result<Json<serde_json::Value>> {
    state
        .tee_service
        .emit_event(&req.event, &req.payload)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to emit event: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Event '{}' emitted successfully", req.event)
    })))
}
