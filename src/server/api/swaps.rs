use anyhow::Result;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::server::db;
use crate::server::models::Swap;
use crate::server::schema::swaps;
use crate::server::state::AppStateInner;
use crate::types::{SwapListDto, SwapRow};

pub async fn list(
    state: &AppStateInner,
    state_filter: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<SwapListDto> {
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let offset = offset.unwrap_or(0).max(0);
    let mut conn = db::checkout(&state.pool).await?;

    let total: i64 = match state_filter {
        Some("active") => {
            swaps::table
                .filter(swaps::state.eq_any(["active", "in-progress"]))
                .count()
                .get_result(&mut *conn)
                .await?
        }
        Some(s) if s != "all" => {
            swaps::table
                .filter(swaps::state.eq(s))
                .count()
                .get_result(&mut *conn)
                .await?
        }
        _ => swaps::table.count().get_result(&mut *conn).await?,
    };

    let rows: Vec<Swap> = match state_filter {
        Some("active") => {
            swaps::table
                .filter(swaps::state.eq_any(["active", "in-progress"]))
                .select(Swap::as_select())
                .order(swaps::started_at.desc())
                .limit(limit)
                .offset(offset)
                .load(&mut *conn)
                .await?
        }
        Some(s) if s != "all" => {
            swaps::table
                .filter(swaps::state.eq(s))
                .select(Swap::as_select())
                .order(swaps::started_at.desc())
                .limit(limit)
                .offset(offset)
                .load(&mut *conn)
                .await?
        }
        _ => {
            swaps::table
                .select(Swap::as_select())
                .order(swaps::started_at.desc())
                .limit(limit)
                .offset(offset)
                .load(&mut *conn)
                .await?
        }
    };

    let rows = rows
        .into_iter()
        .map(|s| SwapRow {
            swap_id: s.swap_id,
            peer_id: s.peer_id,
            state: s.state,
            btc_sat: s.btc_sat,
            xmr_atomic: s.xmr_atomic.to_string(),
            started_at: s.started_at,
            completed_at: s.completed_at,
            profit_usd: s.profit_usd.map(|d| d.round_dp(2).to_string()),
        })
        .collect();

    Ok(SwapListDto { total, rows })
}
