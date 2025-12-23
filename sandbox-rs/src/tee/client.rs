use dstack_sdk::dstack_client::{
    DstackClient, GetKeyResponse, GetQuoteResponse, InfoResponse, SignResponse, VerifyResponse,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct TeeService {
    client: Arc<DstackClient>,
}

impl TeeService {
    pub fn new(endpoint: Option<&str>) -> Self {
        Self {
            client: Arc::new(DstackClient::new(endpoint)),
        }
    }

    pub async fn info(&self) -> anyhow::Result<InfoResponse> {
        self.client.info().await
    }

    pub async fn get_quote(&self, report_data: &[u8]) -> anyhow::Result<GetQuoteResponse> {
        // DstackClient.get_quote() requires Vec<u8> as it consumes the data for hex encoding
        self.client.get_quote(report_data.to_vec()).await
    }

    pub async fn derive_key(&self, path: Option<&str>, purpose: Option<&str>) -> anyhow::Result<GetKeyResponse> {
        self.client.get_key(
            path.map(|s| s.to_string()),
            purpose.map(|s| s.to_string())
        ).await
    }

    pub async fn sign(&self, algorithm: &str, data: &[u8]) -> anyhow::Result<SignResponse> {
        // DstackClient.sign() requires Vec<u8> as it consumes the data for hex encoding
        self.client.sign(algorithm, data.to_vec()).await
    }

    pub async fn verify(&self, algorithm: &str, data: &[u8], signature: &[u8], public_key: &[u8]) -> anyhow::Result<VerifyResponse> {
        // DstackClient.verify() requires Vec<u8> for all byte parameters as it consumes them for hex encoding
        self.client.verify(
            algorithm,
            data.to_vec(),
            signature.to_vec(),
            public_key.to_vec()
        ).await
    }

    pub async fn emit_event(&self, event: &str, payload: &str) -> anyhow::Result<()> {
        // DstackClient.emit_event() requires Vec<u8> payload as it consumes it for hex encoding
        self.client.emit_event(
            event.to_string(),
            payload.as_bytes().to_vec()
        ).await
    }
}
