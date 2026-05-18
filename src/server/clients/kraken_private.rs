//! Kraken **private** REST API client (read-only). Used by the balance-snapshot
//! poller to record Kraken-side holdings alongside the maker's on-wallet
//! balances, so the chart's total value doesn't dip during a recycle when
//! BTC has left the maker but hasn't yet come back as XMR.
//!
//! Auth scheme: every request includes a millisecond `nonce` in the POST body
//! and an `API-Sign` header = base64(HMAC-SHA512(secret, uri_path ||
//! SHA256(nonce || postdata))) where `secret` is the base64-decoded API
//! secret. Same scheme as `homelab/scripts/kraken-query.py`.
//!
//! Permissions required on the API key: **Query Funds**. Nothing else.
//! Never accept a key with Trade or Withdraw scopes here.

use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use hmac::{Hmac, Mac};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;

const API_HOST: &str = "https://api.kraken.com";

#[derive(Clone)]
pub struct KrakenPrivateClient {
    http: Client,
    api_key: String,
    api_secret_b64: String,
}

impl std::fmt::Debug for KrakenPrivateClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never log key/secret values.
        f.debug_struct("KrakenPrivateClient")
            .field("host", &API_HOST)
            .finish()
    }
}

/// Aggregated balances broken down into the fields we feed into a snapshot.
/// All quantities in atomic / USD units to match the schema columns.
#[derive(Debug, Default, Clone)]
pub struct KrakenBalances {
    pub btc_sat: i64,
    pub xmr_atomic: Decimal,
    pub usd: Decimal,
}

impl KrakenPrivateClient {
    /// Construct from explicit credentials. Returns `None` if either is empty
    /// — caller can treat that as "Kraken integration disabled".
    pub fn new(api_key: String, api_secret_b64: String) -> Option<Self> {
        if api_key.trim().is_empty() || api_secret_b64.trim().is_empty() {
            return None;
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("eigenwallet-admin/kraken-readonly/1")
            .build()
            .expect("build kraken http client");
        Some(Self {
            http,
            api_key,
            api_secret_b64,
        })
    }

    /// Returns the raw `Balance` map keyed by Kraken's asset codes
    /// (`XXBT`, `XXMR`, `USDT`, `ZUSD`, ...). Values are atomic-precision
    /// decimal strings; we parse them as `Decimal`.
    pub async fn balance(&self) -> Result<HashMap<String, Decimal>> {
        let nonce = chrono::Utc::now().timestamp_millis().to_string();
        let postdata = format!("nonce={nonce}");
        let uri_path = "/0/private/Balance";
        let signature = sign(uri_path, &nonce, &postdata, &self.api_secret_b64)?;

        #[derive(Deserialize)]
        struct Envelope {
            error: Vec<String>,
            result: Option<HashMap<String, Decimal>>,
        }
        let env: Envelope = self
            .http
            .post(format!("{API_HOST}{uri_path}"))
            .header("API-Key", &self.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(postdata)
            .send()
            .await?
            .json()
            .await?;

        if !env.error.is_empty() {
            return Err(anyhow!("kraken Balance error: {:?}", env.error));
        }
        env.result
            .ok_or_else(|| anyhow!("kraken Balance returned null result"))
    }

    /// Convenience: query Balance and project into the snapshot fields.
    /// Aggregates `XXBT`/`XBT` for BTC, `XXMR`/`XMR` for XMR, and
    /// `USDT + ZUSD` for the dollar bucket. Other assets (bonds, staking
    /// derivatives) are ignored.
    pub async fn snapshot_balances(&self) -> Result<KrakenBalances> {
        let raw = self.balance().await?;
        let get = |k: &str| raw.get(k).copied().unwrap_or(Decimal::ZERO);

        let btc = get("XXBT") + get("XBT");
        let xmr = get("XXMR") + get("XMR");
        let usd = get("USDT") + get("ZUSD");

        let btc_sat: i64 = (btc * Decimal::from(100_000_000i64))
            .trunc()
            .try_into()
            .unwrap_or(0);
        let xmr_atomic = (xmr * Decimal::from(1_000_000_000_000i64)).trunc();

        Ok(KrakenBalances {
            btc_sat,
            xmr_atomic,
            usd,
        })
    }
}

fn sign(uri_path: &str, nonce: &str, postdata: &str, secret_b64: &str) -> Result<String> {
    // Step 1: SHA-256 over (nonce || postdata) — note postdata already
    // begins with `nonce=...` so the nonce appears twice, which is Kraken's
    // documented behavior.
    let mut sha256 = Sha256::new();
    sha256.update(nonce.as_bytes());
    sha256.update(postdata.as_bytes());
    let sha256_digest = sha256.finalize();

    // Step 2: HMAC-SHA512 over (uri_path || sha256_digest) keyed by
    // base64-decoded secret.
    let secret_bytes = B64
        .decode(secret_b64.trim())
        .map_err(|e| anyhow!("kraken secret is not valid base64: {e}"))?;
    let mut mac = <Hmac<Sha512>>::new_from_slice(&secret_bytes)
        .map_err(|e| anyhow!("hmac key length error: {e}"))?;
    mac.update(uri_path.as_bytes());
    mac.update(&sha256_digest);

    Ok(B64.encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_kraken_doc_vector() {
        // Test vector from Kraken's API docs:
        //   https://docs.kraken.com/api/docs/guides/spot-rest-auth
        // Inputs:
        //   secret = "kQH5HW/8p1uGOVjbgWA7FunAmGO8lsSUXNsu3eow76sz84Q18fWxnyRzBHCd3pd5nE9qa99HAZtuZuj6F1huXg=="
        //   uri_path = "/0/private/AddOrder"
        //   nonce = "1616492376594"
        //   postdata = "nonce=1616492376594&ordertype=limit&pair=XBTUSD&price=37500&type=buy&volume=1.25"
        // Expected:
        //   "4/dpxb3iT4tp/ZCVEwSnEsLxx0bqyhLpdfOpc6fn7OR8+UClSV5n9E6aSS8MPtnRfp32bAb0nmbRn6H8ndwLUQ=="
        let secret = "kQH5HW/8p1uGOVjbgWA7FunAmGO8lsSUXNsu3eow76sz84Q18fWxnyRzBHCd3pd5nE9qa99HAZtuZuj6F1huXg==";
        let uri = "/0/private/AddOrder";
        let nonce = "1616492376594";
        let postdata =
            "nonce=1616492376594&ordertype=limit&pair=XBTUSD&price=37500&type=buy&volume=1.25";
        let sig = sign(uri, nonce, postdata, secret).unwrap();
        assert_eq!(
            sig,
            "4/dpxb3iT4tp/ZCVEwSnEsLxx0bqyhLpdfOpc6fn7OR8+UClSV5n9E6aSS8MPtnRfp32bAb0nmbRn6H8ndwLUQ=="
        );
    }
}
