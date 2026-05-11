//! Operator-edited address registry. Lives in the `wallet-rules` ConfigMap in
//! the eigenwallet namespace; admin reads on startup and re-reads every 60 s.
//!
//! ConfigMap layout (YAML, one document):
//!
//! ```yaml
//! addresses:
//!   - addr: "bc1q..."
//!     kind: taker            # taker | cold-storage | exchange | other
//!     asset: BTC             # BTC | XMR
//!     label: "swap-cli taker BTC deposit"
//!     note: "internal flow — not a capital event"
//!   - addr: "bc1q..."
//!     kind: cold-storage
//!     asset: BTC
//!     label: "main BTC vault"
//! ```
//!
//! Classifying logic (used by future capital-event auto-tagging):
//!   - `taker` addresses: transactions to/from these are internal recycle
//!     flow; NEVER a capital event.
//!   - `cold-storage`: deposits FROM = topping up inventory; withdrawals TO =
//!     pulling profit. Both are capital events.
//!   - `exchange`: similar to cold-storage but tagged separately for reporting.
//!   - `other`: known but not auto-classified.
//!
//! Edit with: `kubectl edit configmap wallet-rules -n eigenwallet`. Updates
//! pick up within ~60 s without restarting the admin pod.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::server::state::AppStateInner;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressEntry {
    pub addr: String,
    pub kind: String,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WalletRules {
    #[serde(default)]
    pub addresses: Vec<AddressEntry>,
}

impl WalletRules {
    pub fn classify<'a>(&'a self, addr: &str) -> Option<&'a AddressEntry> {
        self.addresses.iter().find(|e| e.addr == addr)
    }

    pub fn is_internal(&self, addr: &str) -> bool {
        self.classify(addr).is_some_and(|e| e.kind == "taker")
    }
}

#[derive(Debug, Default)]
pub struct WalletRulesCache {
    pub rules: WalletRules,
    pub last_loaded: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

pub type WalletRulesHandle = Arc<RwLock<WalletRulesCache>>;

const CONFIGMAP_NAME: &str = "wallet-rules";
const CONFIGMAP_KEY: &str = "rules.yaml";

/// Read the ConfigMap and parse the YAML payload. Missing ConfigMap → empty
/// rules (not an error — the system works without it; auto-classification
/// just won't trigger).
pub async fn load_rules(state: &AppStateInner) -> Result<WalletRules> {
    let Some(kube) = state.kube.as_ref() else {
        return Ok(WalletRules::default());
    };
    let cm = match kube
        .read_configmap(&state.config.asb_namespace, CONFIGMAP_NAME)
        .await
    {
        Ok(cm) => cm,
        Err(e) => {
            let s = e.to_string();
            if s.contains("NotFound") || s.contains("not found") {
                return Ok(WalletRules::default());
            }
            return Err(e).context("read wallet-rules configmap");
        }
    };
    let raw = cm
        .data
        .as_ref()
        .and_then(|m| m.get(CONFIGMAP_KEY).cloned())
        .unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(WalletRules::default());
    }
    serde_yaml_ng::from_str(&raw).map_err(|e| anyhow!("parse wallet-rules YAML: {e}"))
}

pub async fn refresh(handle: WalletRulesHandle, state: Arc<AppStateInner>) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        tick.tick().await;
        match load_rules(&state).await {
            Ok(rules) => {
                let mut w = handle.write().await;
                w.rules = rules;
                w.last_loaded = Some(chrono::Utc::now());
                w.last_error = None;
            }
            Err(e) => {
                let mut w = handle.write().await;
                w.last_error = Some(e.to_string());
                tracing::warn!(error = %e, "wallet-rules refresh failed");
            }
        }
    }
}
