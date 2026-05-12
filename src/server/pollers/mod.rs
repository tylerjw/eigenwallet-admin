//! Background pollers spawned at startup. Each loop owns its own interval
//! and never tries to retry within a tick — failures log and wait for next.

mod balance_snapshot;
mod cex_prices;
mod competitor_scan;
mod kraken_backfill;
mod log_tail;
mod swap_sync;

use crate::server::state::AppState;

pub fn spawn_all(state: AppState) {
    tokio::spawn(cex_prices::run(state.clone()));
    tokio::spawn(balance_snapshot::run(state.clone()));
    tokio::spawn(swap_sync::run(state.clone()));
    tokio::spawn(log_tail::run(state.clone()));
    tokio::spawn(crate::server::wallet_rules::refresh(
        state.wallet_rules.clone(),
        state.0.clone(),
    ));
    let backfill_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = kraken_backfill::run_once(backfill_state).await {
            tracing::warn!(error = %e, "kraken ohlc backfill failed");
        }
    });
    tokio::spawn(competitor_scan::run(state));
}
