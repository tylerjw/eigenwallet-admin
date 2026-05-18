//! Periodically pull `get_swaps` and upsert rows. asb's `get_swaps` returns
//! both active and completed swaps with full amounts, peer_id, and start_date,
//! so this is the canonical sync — no separate log-tail needed for state.

use std::time::Duration;

use chrono::Utc;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::clients::asb::{SwapEntry, parse_swap_start_date};
use crate::server::db;
use crate::server::models::Swap;
use crate::server::schema::swaps;
use crate::server::state::AppState;

pub async fn run(state: AppState) {
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tick.tick().await;
        match state.asb.get_swaps().await {
            Ok(entries) => {
                if let Err(e) = upsert(&state, entries).await {
                    tracing::warn!(error = %e, "swap upsert failed");
                }
            }
            Err(e) => tracing::warn!(error = %e, "asb get_swaps failed"),
        }
    }
}

async fn upsert(state: &AppState, entries: Vec<SwapEntry>) -> anyhow::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut conn = db::checkout(&state.pool).await?;
    for e in entries {
        let started_at = parse_swap_start_date(&e.start_date)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let xmr_atomic: Decimal = e.xmr_amount.to_string().parse().unwrap_or(Decimal::ZERO);
        let completed_at = if e.completed { Some(started_at) } else { None };
        let row = Swap {
            swap_id: e.swap_id,
            peer_id: e.peer_id,
            state: e.state,
            btc_sat: e.btc_amount,
            xmr_atomic,
            started_at,
            completed_at,
            btc_usd_at_completion: None,
            xmr_usd_at_completion: None,
            profit_usd: None,
            raw_log_excerpt: None,
        };
        diesel::insert_into(swaps::table)
            .values(&row)
            .on_conflict(swaps::swap_id)
            .do_update()
            .set((
                swaps::state.eq(excluded(swaps::state)),
                swaps::btc_sat.eq(excluded(swaps::btc_sat)),
                swaps::xmr_atomic.eq(excluded(swaps::xmr_atomic)),
                swaps::completed_at.eq(excluded(swaps::completed_at)),
            ))
            .execute(&mut *conn)
            .await?;
    }
    // Compute profit_usd for any newly-completed swaps. Looks up the nearest
    // cex_prices sample to completed_at and applies the maker accounting:
    //   - redeemed: maker received BTC, paid XMR → profit = btc·btc_usd − xmr·xmr_usd
    //   - punished: maker kept BTC and own XMR  → profit = btc·btc_usd
    //   - refunded / other: zero (no value exchanged)
    // Idempotent: WHERE profit_usd IS NULL skips already-populated rows.
    diesel::sql_query(
        "UPDATE swaps s SET \
           btc_usd_at_completion = (SELECT btc_usd FROM cex_prices \
              WHERE btc_usd IS NOT NULL \
              ORDER BY abs(EXTRACT(epoch FROM (sampled_at - s.completed_at))) ASC LIMIT 1), \
           xmr_usd_at_completion = (SELECT xmr_usd FROM cex_prices \
              WHERE xmr_usd IS NOT NULL \
              ORDER BY abs(EXTRACT(epoch FROM (sampled_at - s.completed_at))) ASC LIMIT 1), \
           profit_usd = CASE \
             WHEN s.state ILIKE '%redeemed%' THEN \
               s.btc_sat::numeric/1e8 * \
                 (SELECT btc_usd FROM cex_prices WHERE btc_usd IS NOT NULL \
                  ORDER BY abs(EXTRACT(epoch FROM (sampled_at - s.completed_at))) ASC LIMIT 1) \
               - s.xmr_atomic::numeric/1e12 * \
                 (SELECT xmr_usd FROM cex_prices WHERE xmr_usd IS NOT NULL \
                  ORDER BY abs(EXTRACT(epoch FROM (sampled_at - s.completed_at))) ASC LIMIT 1) \
             WHEN s.state ILIKE '%punished%' THEN \
               s.btc_sat::numeric/1e8 * \
                 (SELECT btc_usd FROM cex_prices WHERE btc_usd IS NOT NULL \
                  ORDER BY abs(EXTRACT(epoch FROM (sampled_at - s.completed_at))) ASC LIMIT 1) \
             ELSE 0 END \
         WHERE s.completed_at IS NOT NULL AND s.profit_usd IS NULL",
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}
