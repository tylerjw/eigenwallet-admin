//! Every 5 minutes: pull asb balances, multiply by latest CEX prices, insert row.

use std::time::Duration;

use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::models::BalanceSnapshot;
use crate::server::schema::balance_snapshots;
use crate::server::state::AppState;

pub async fn run(state: AppState) {
    let mut tick = tokio::time::interval(Duration::from_secs(300));
    loop {
        tick.tick().await;
        if let Err(e) = snapshot_once(&state).await {
            tracing::warn!(error = %e, "balance snapshot failed");
        }
    }
}

async fn snapshot_once(state: &AppState) -> anyhow::Result<()> {
    let btc = state.asb.bitcoin_balance().await?;
    let xmr = state.asb.monero_balance().await?;
    let xmr_atomic: Decimal = xmr.balance.to_string().parse().unwrap_or(Decimal::ZERO);

    let snap_cex = {
        let cache = state.cex.read().await;
        cache.last.clone()
    };
    let (btc_usd, xmr_usd) = match &snap_cex {
        Some(s) => (
            s.btc_usd.unwrap_or(Decimal::ZERO),
            s.xmr_usd.unwrap_or(Decimal::ZERO),
        ),
        None => (Decimal::ZERO, Decimal::ZERO),
    };

    // Skip rows we can't price. A snapshot with zero prices propagates as
    // total_usd=0 and poisons lifetime-ROI / chart math downstream. The
    // CEX poller will recover on its next tick.
    if btc_usd.is_zero() || xmr_usd.is_zero() {
        tracing::debug!("balance-snapshot: skipping row, CEX prices unavailable");
        return Ok(());
    }

    let btc_decimal = Decimal::from(btc.balance) / Decimal::from(100_000_000i64);
    let xmr_decimal = xmr_atomic / Decimal::from(1_000_000_000_000i64);
    let total_btc = btc_decimal
        + if !btc_usd.is_zero() {
            xmr_decimal * xmr_usd / btc_usd
        } else {
            Decimal::ZERO
        };
    let total_usd = btc_decimal * btc_usd + xmr_decimal * xmr_usd;

    let row = BalanceSnapshot {
        taken_at: chrono::Utc::now(),
        btc_sat: btc.balance,
        xmr_atomic,
        btc_usd,
        xmr_usd,
        total_usd,
        total_btc,
    };
    let mut conn = db::checkout(&state.pool).await?;
    diesel::insert_into(balance_snapshots::table)
        .values(&row)
        .on_conflict(balance_snapshots::taken_at)
        .do_nothing()
        .execute(&mut *conn)
        .await?;
    Ok(())
}
