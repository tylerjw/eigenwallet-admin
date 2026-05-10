//! Tiny Electrum protocol client. We only need `blockchain.headers.subscribe`
//! to learn the current Bitcoin tip height.

use anyhow::{Result, anyhow};

#[derive(Clone, Debug)]
pub struct ElectrsClient {
    addr: String,
}

impl ElectrsClient {
    pub fn new(url: &str) -> Self {
        let addr = url.strip_prefix("tcp://").unwrap_or(url).to_string();
        Self { addr }
    }

    pub async fn tip_height(&self) -> Result<i64> {
        // electrum-client is synchronous; do it on the blocking pool.
        let addr = self.addr.clone();
        tokio::task::spawn_blocking(move || -> Result<i64> {
            use electrum_client::ElectrumApi;
            let client = electrum_client::Client::new(&addr)
                .map_err(|e| anyhow!("electrum connect {addr}: {e}"))?;
            let head = client
                .block_headers_subscribe()
                .map_err(|e| anyhow!("headers_subscribe: {e}"))?;
            Ok(head.height as i64)
        })
        .await
        .map_err(|e| anyhow!("electrs join: {e}"))?
    }
}
