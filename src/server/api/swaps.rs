use anyhow::Result;
use chrono::{DateTime, Utc};
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Bool;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::models::{CexPrice, Swap};
use crate::server::schema::{cex_prices, swaps};
use crate::server::state::AppStateInner;
use crate::types::{SwapListDto, SwapRow};

/// Translate a UI filter token into a SQL predicate on `swaps.state` +
/// `swaps.completed_at`. asb's state strings are free-text English (e.g.
/// `"btc is redeemed"`, `"btc is early refunded"`, `"btc is punished"`,
/// `"xmr is refunded"`) so we substring-match by category.
fn build_state_predicate(
    filter: &str,
) -> Option<Box<dyn diesel::BoxableExpression<swaps::table, diesel::pg::Pg, SqlType = Bool>>> {
    match filter {
        "active" => Some(Box::new(swaps::completed_at.is_null())),
        "completed" => Some(Box::new(
            swaps::completed_at
                .is_not_null()
                .and(swaps::state.like("%redeemed%")),
        )),
        "refunded" => Some(Box::new(swaps::state.like("%refunded%"))),
        "punished" => Some(Box::new(swaps::state.like("%punished%"))),
        _ => None,
    }
}

pub async fn list(
    state: &AppStateInner,
    state_filter: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<SwapListDto> {
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let offset = offset.unwrap_or(0).max(0);
    let mut conn = db::checkout(&state.pool).await?;

    let predicate = state_filter.and_then(build_state_predicate);

    let total: i64 = match &predicate {
        Some(p) => {
            // Boxed predicates can't be reused, so rebuild for the count query.
            let _ = p;
            let p2 = build_state_predicate(state_filter.unwrap()).unwrap();
            swaps::table
                .filter(p2)
                .count()
                .get_result(&mut *conn)
                .await?
        }
        None => swaps::table.count().get_result(&mut *conn).await?,
    };

    let rows: Vec<Swap> = match predicate {
        Some(p) => {
            swaps::table
                .filter(p)
                .select(Swap::as_select())
                .order(swaps::started_at.desc())
                .limit(limit)
                .offset(offset)
                .load(&mut *conn)
                .await?
        }
        None => {
            swaps::table
                .select(Swap::as_select())
                .order(swaps::started_at.desc())
                .limit(limit)
                .offset(offset)
                .load(&mut *conn)
                .await?
        }
    };

    // Pre-fetch all CEX prices once for an in-memory join. ~1 row per minute,
    // ~1440/day, so even a year of data is < 600k rows — cheap to load and
    // avoids N+1 round trips.
    let prices: Vec<CexPrice> = cex_prices::table
        .order(cex_prices::sampled_at.asc())
        .load(&mut *conn)
        .await?;

    let rows = rows
        .into_iter()
        .map(|s| {
            // If the DB has a cached profit_usd, use it. Otherwise compute
            // from the completed-at timestamp + nearest CEX snapshot.
            let profit = s.profit_usd.or_else(|| compute_profit_usd(&s, &prices));
            SwapRow {
                swap_id: s.swap_id,
                peer_id: s.peer_id,
                state: s.state,
                btc_sat: s.btc_sat,
                xmr_atomic: s.xmr_atomic.to_string(),
                started_at: s.started_at,
                completed_at: s.completed_at,
                profit_usd: profit.map(|d| d.round_dp(2).to_string()),
            }
        })
        .collect();

    Ok(SwapListDto { total, rows })
}

/// Compute profit_usd for a completed maker swap.
///
/// Maker semantics (asb is the XMR seller): we received `btc_amount` and sent
/// `xmr_amount`. Profit is the spread captured vs. a hypothetical CEX-mid
/// trade at the time of completion:
///
///   profit_btc = btc_received - xmr_sent × cex_btc_per_xmr
///   profit_usd = profit_btc × cex_btc_usd
///
/// For refunds: btc_amount returned to taker, no profit captured;
/// profit_usd is just the negative of the swap fees paid (approximated as 0
/// for v1).
/// For punishments: we kept the BTC AND the XMR locked, so it's a gross win;
/// profit ≈ value of the lock_collateral. Out of scope for v1 — return None.
fn compute_profit_usd(s: &Swap, prices: &[CexPrice]) -> Option<Decimal> {
    let completed_at = s.completed_at?;
    if !s.state.contains("redeemed") {
        // Refund or punish — profit math is different and we don't have
        // enough state to compute it accurately yet. Show as "—".
        return None;
    }
    let p = nearest_cex_price(prices, completed_at)?;
    let btc_xmr = p.btc_xmr?;
    let btc_usd = p.btc_usd?;
    let btc = Decimal::from(s.btc_sat) / Decimal::from(100_000_000i64);
    let xmr = s.xmr_atomic / Decimal::from(1_000_000_000_000i64);
    let fair_btc_for_xmr = xmr * btc_xmr;
    let profit_btc = btc - fair_btc_for_xmr;
    Some(profit_btc * btc_usd)
}

fn nearest_cex_price(prices: &[CexPrice], at: DateTime<Utc>) -> Option<&CexPrice> {
    if prices.is_empty() {
        return None;
    }
    // Linear scan; prices is sorted asc by sampled_at. A binary search would
    // be O(log n) but n is small.
    let mut best: Option<&CexPrice> = None;
    let mut best_gap = i64::MAX;
    for p in prices {
        let gap = (p.sampled_at - at).num_seconds().abs();
        if gap < best_gap {
            best_gap = gap;
            best = Some(p);
        }
    }
    // If the nearest sample is more than 2 hours away, we don't trust it.
    if best_gap > 7200 { None } else { best }
}

// Mark `sql` as used so clippy doesn't complain when no SQL literal is used.
#[allow(dead_code)]
fn _keep_sql_imported() -> diesel::expression::SqlLiteral<Bool> {
    sql("true")
}
