use anyhow::Result;

use crate::server::state::AppStateInner;
use crate::types::{TaggedAddressDto, WalletRulesDto};

pub async fn fetch(state: &AppStateInner) -> Result<WalletRulesDto> {
    let snap = state.wallet_rules.read().await;
    Ok(WalletRulesDto {
        addresses: snap
            .rules
            .addresses
            .iter()
            .map(|e| TaggedAddressDto {
                addr: e.addr.clone(),
                kind: e.kind.clone(),
                asset: e.asset.clone(),
                label: e.label.clone(),
                note: e.note.clone(),
            })
            .collect(),
        last_loaded: snap.last_loaded,
        last_error: snap.last_error.clone(),
    })
}
