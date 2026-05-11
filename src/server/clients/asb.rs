//! Typed wrapper over asb 4.5.0's JSON-RPC at port 9944.
//!
//! Method names and response schemas verified against the running asb-4.5.0
//! in the homelab on 2026-05-10. Notable shapes:
//!   - All amount fields (`balance`, `btc_amount`, `xmr_amount`, `price`,
//!     `min_quantity`, `max_quantity`, `exchange_rate`, `btc_redeem_fee`) are
//!     integers in the asset's atomic units (satoshi for BTC, piconero/atomic
//!     for XMR, sat-per-XMR for `price` and `exchange_rate`).
//!   - `start_date` uses chrono's default `Display` format
//!     (`"2026-04-19 10:52:14.854013895 +00:00:00"`) — parse with the helper
//!     `parse_swap_start_date` below.
//!   - `registration_status` returns per-entry `connection` and `registration`
//!     enums as bare strings.

use anyhow::{Result, anyhow};
use chrono::{DateTime, FixedOffset};
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::HttpClient;
use jsonrpsee::rpc_params;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AsbClient {
    inner: HttpClient,
    url: String,
}

impl std::fmt::Debug for AsbClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsbClient").field("url", &self.url).finish()
    }
}

impl AsbClient {
    pub fn new(url: &str) -> Self {
        let inner = HttpClient::builder()
            .request_timeout(std::time::Duration::from_secs(15))
            .build(url)
            .expect("build asb http client");
        Self {
            inner,
            url: url.to_string(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn bitcoin_balance(&self) -> Result<BitcoinBalance> {
        self.call("bitcoin_balance", rpc_params![]).await
    }

    pub async fn monero_balance(&self) -> Result<MoneroBalance> {
        self.call("monero_balance", rpc_params![]).await
    }

    /// Active and historical swaps known to this asb. `state` is a free-text
    /// label (e.g. `"btc is redeemed"`); `completed` is true for terminal swaps.
    pub async fn get_swaps(&self) -> Result<Vec<SwapEntry>> {
        self.call("get_swaps", rpc_params![]).await
    }

    pub async fn registration_status(&self) -> Result<RegistrationStatus> {
        self.call("registration_status", rpc_params![]).await
    }

    /// Current quote we're advertising. Prices and quantities are in satoshi.
    pub async fn get_current_quote(&self) -> Result<Quote> {
        self.call("get_current_quote", rpc_params![]).await
    }

    pub async fn multiaddresses(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct R {
            multiaddresses: Vec<String>,
        }
        let r: R = self.call("multiaddresses", rpc_params![]).await?;
        Ok(r.multiaddresses)
    }

    pub async fn active_connections(&self) -> Result<i32> {
        #[derive(Deserialize)]
        struct R {
            connections: i32,
        }
        let r: R = self.call("active_connections", rpc_params![]).await?;
        Ok(r.connections)
    }

    pub async fn check_connection(&self) -> Result<bool> {
        // Returns null on success.
        let v: serde_json::Value = self
            .call("check_connection", rpc_params![])
            .await
            .unwrap_or(serde_json::Value::Null);
        Ok(matches!(v, serde_json::Value::Null) || v.is_object())
    }

    pub async fn peer_id(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct R {
            peer_id: String,
        }
        let r: R = self.call("peer_id", rpc_params![]).await?;
        Ok(r.peer_id)
    }

    pub async fn onion_service_status(&self) -> Result<OnionStatus> {
        self.call("onion_service_status", rpc_params![]).await
    }

    async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: jsonrpsee::core::params::ArrayParams,
    ) -> Result<T> {
        self.inner
            .request::<T, _>(method, params)
            .await
            .map_err(|e| anyhow!("asb RPC {method}: {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinBalance {
    /// Spendable BTC in satoshi.
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneroBalance {
    /// Spendable XMR in atomic units (piconero, 1 XMR = 1e12).
    pub balance: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEntry {
    pub swap_id: String,
    /// chrono Display-format timestamp; use `parse_swap_start_date` to parse.
    pub start_date: String,
    pub state: String,
    pub peer_id: String,
    #[serde(default)]
    pub btc_amount: i64,
    #[serde(default)]
    pub xmr_amount: u128,
    /// Negotiated exchange rate in satoshi per 1 XMR.
    #[serde(default)]
    pub exchange_rate: i64,
    #[serde(default)]
    pub btc_redeem_fee: i64,
    #[serde(default)]
    pub btc_lock_txid: Option<String>,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationStatus {
    pub registrations: Vec<RegistrationEntry>,
}

impl RegistrationStatus {
    pub fn registered_count(&self) -> i32 {
        self.registrations
            .iter()
            .filter(|e| e.is_registered())
            .count() as i32
    }
    pub fn total(&self) -> i32 {
        self.registrations.len() as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationEntry {
    /// Rendezvous multiaddr.
    pub address: String,
    /// Bare enum variant: "Connected", "Disconnected".
    pub connection: String,
    /// Bare enum variant: "Registered", "RegisterOnceConnected", others.
    pub registration: String,
}

impl RegistrationEntry {
    pub fn is_registered(&self) -> bool {
        self.registration.eq_ignore_ascii_case("registered")
    }
    pub fn is_connected(&self) -> bool {
        self.connection.eq_ignore_ascii_case("connected")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    /// Satoshi per 1 XMR.
    pub price: i64,
    /// Minimum buy amount, satoshi.
    pub min_quantity: i64,
    /// Maximum buy amount, satoshi (constrained by current inventory).
    pub max_quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnionStatus {
    /// e.g. "Bootstrapping", "DegradedUnreachable", "Available".
    pub state: String,
    pub reachable: bool,
    #[serde(default)]
    pub problem: Option<String>,
}

/// Parse asb's swap `start_date` field. Format:
///   "2026-04-19 10:52:14.854013895 +00:00:00"
///
/// This is chrono's own `DateTime<FixedOffset>` Display output, but chrono
/// can't parse it back: its `%::z` specifier accepts `+HH:MM` (not the
/// `+HH:MM:SS` form chrono itself produces). Strip the trailing `:SS` from
/// the offset and parse with `%:z`.
pub fn parse_swap_start_date(s: &str) -> Option<DateTime<FixedOffset>> {
    let trimmed = strip_offset_seconds(s);
    DateTime::parse_from_str(&trimmed, "%Y-%m-%d %H:%M:%S%.f %:z").ok()
}

fn strip_offset_seconds(s: &str) -> String {
    // Split off the offset (everything after the last whitespace) and, if it
    // has two colons (`+HH:MM:SS`), drop the last `:XX`.
    let Some(sp) = s.rfind(char::is_whitespace) else {
        return s.to_string();
    };
    let (head, tail) = s.split_at(sp);
    let offset = tail.trim_start();
    if offset.matches(':').count() == 2
        && let Some(last_colon) = offset.rfind(':')
    {
        return format!("{head} {}", &offset[..last_colon]);
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_swap_start_date_with_seconds_offset() {
        let dt = parse_swap_start_date("2026-04-19 10:52:14.854013895 +00:00:00");
        assert!(dt.is_some(), "expected a parsed DateTime, got None");
        assert_eq!(
            dt.unwrap().to_rfc3339(),
            "2026-04-19T10:52:14.854013895+00:00"
        );
    }

    #[test]
    fn parses_swap_start_date_with_two_segment_offset() {
        let dt = parse_swap_start_date("2026-04-19 10:52:14.123 +00:00");
        assert!(dt.is_some());
    }
}
