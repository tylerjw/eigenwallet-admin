use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct MonerodClient {
    base: String,
    http: Client,
}

impl MonerodClient {
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .expect("monerod http client"),
        }
    }

    /// Calls `get_info` on monerod's JSON-RPC endpoint.
    pub async fn get_info(&self) -> Result<MonerodInfo> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "ewa",
            "method": "get_info",
            "params": {}
        });
        let url = format!("{}/json_rpc", self.base);
        let resp: serde_json::Value = self.http.post(url).json(&body).send().await?.json().await?;
        let result = resp
            .get("result")
            .ok_or_else(|| anyhow!("monerod: no result"))?;
        serde_json::from_value(result.clone()).map_err(|e| anyhow!("monerod parse: {e}"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonerodInfo {
    #[serde(default)]
    pub height: u64,
    #[serde(default)]
    pub target_height: u64,
    #[serde(default)]
    pub synchronized: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub mainnet: bool,
}
