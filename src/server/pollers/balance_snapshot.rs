//! Every 5 minutes: pull asb + Kraken balances, multiply by latest CEX prices,
//! insert a row.
//!
//! Including the Kraken-side holdings is what prevents the chart from showing
//! a phantom "loss" during a recycle: when BTC has been withdrawn from the
//! maker wallet but is still sitting at Kraken (pre-trade) or has been
//! converted to USDT/XMR (mid-recycle), the operator still owns the value —
//! it's just held outside the asb wallet for a few hours. See `docs/RECYCLE.md`.

use std::time::Duration;

use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::clients::kraken_private::KrakenBalances;
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
    // Run asb and Kraken queries concurrently — they're independent and we
    // don't want the slower one to delay the snapshot.
    let asb_btc_fut = state.asb.bitcoin_balance();
    let asb_xmr_fut = state.asb.monero_balance();
    let kraken_fut = fetch_kraken(state);

    let (btc, xmr, kraken) = tokio::join!(asb_btc_fut, asb_xmr_fut, kraken_fut);
    let btc = btc?;
    let xmr = xmr?;
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

    let asb_btc = Decimal::from(btc.balance) / Decimal::from(100_000_000i64);
    let asb_xmr = xmr_atomic / Decimal::from(1_000_000_000_000i64);
    let kraken_btc = Decimal::from(kraken.btc_sat) / Decimal::from(100_000_000i64);
    let kraken_xmr = kraken.xmr_atomic / Decimal::from(1_000_000_000_000i64);

    // Total value spans asb wallet + Kraken account. kraken_usd is already
    // in USD (USDT + ZUSD aggregated by the client).
    let total_usd =
        (asb_btc + kraken_btc) * btc_usd + (asb_xmr + kraken_xmr) * xmr_usd + kraken.usd;
    let total_btc = if !btc_usd.is_zero() {
        (asb_btc + kraken_btc) + ((asb_xmr + kraken_xmr) * xmr_usd + kraken.usd) / btc_usd
    } else {
        Decimal::ZERO
    };

    let row = BalanceSnapshot {
        taken_at: chrono::Utc::now(),
        btc_sat: btc.balance,
        xmr_atomic,
        btc_usd,
        xmr_usd,
        total_usd,
        total_btc,
        kraken_btc_sat: kraken.btc_sat,
        kraken_xmr_atomic: kraken.xmr_atomic,
        kraken_usd: kraken.usd,
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

/// Query Kraken if a client is configured; return zeros (logged at debug) on
/// any error so the snapshot still goes in. A persistent failure here is
/// worth investigating — `kraken_*` columns going to zero for hours means
/// the chart is back to the asb-only view, but it doesn't break anything.
async fn fetch_kraken(state: &AppState) -> KrakenBalances {
    let Some(client) = state.kraken.as_ref() else {
        return KrakenBalances::default();
    };
    match client.snapshot_balances().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "kraken balance query failed; recording 0 for kraken_* columns");
            KrakenBalances::default()
        }
    }
}
