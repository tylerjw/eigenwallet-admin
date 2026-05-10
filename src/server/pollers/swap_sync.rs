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
    Ok(())
}
