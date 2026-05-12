use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::schema::{balance_snapshots, capital_events};
use crate::server::state::AppStateInner;
use crate::types::{LifetimeRoiDto, RoiDto};

pub async fn compute(
    state: &AppStateInner,
    since: Option<&str>,
    method: &str,
    denom: &str,
) -> Result<RoiDto> {
    let mut conn = db::checkout(&state.pool).await?;

    let since: DateTime<Utc> = since
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - ChronoDuration::days(30));

    // Latest snapshot
    let latest: Option<(DateTime<Utc>, Decimal, Decimal)> = balance_snapshots::table
        .select((
            balance_snapshots::taken_at,
            balance_snapshots::total_usd,
            balance_snapshots::total_btc,
        ))
        .order(balance_snapshots::taken_at.desc())
        .first(&mut *conn)
        .await
        .optional()?;
    let earliest: Option<(DateTime<Utc>, Decimal, Decimal)> = balance_snapshots::table
        .filter(balance_snapshots::taken_at.ge(since))
        .select((
            balance_snapshots::taken_at,
            balance_snapshots::total_usd,
            balance_snapshots::total_btc,
        ))
        .order(balance_snapshots::taken_at.asc())
        .first(&mut *conn)
        .await
        .optional()?;

    let (start, current) = match (earliest, latest) {
        (Some(e), Some(l)) => (e, l),
        _ => {
            return Ok(RoiDto {
                method: method.into(),
                denomination: denom.into(),
                since,
                start_value: "0".into(),
                current_value: "0".into(),
                pct_change: "0.00".into(),
                days_elapsed: 0,
            });
        }
    };

    let pick = |t: (DateTime<Utc>, Decimal, Decimal)| -> Decimal {
        match denom {
            "btc" => t.2,
            _ => t.1,
        }
    };

    let s = pick(start);
    let c = pick(current);
    let pct = if !s.is_zero() {
        ((c - s) / s * Decimal::from(100)).round_dp(2)
    } else {
        Decimal::ZERO
    };
    let days = (current.0 - start.0).num_days() as i32;

    Ok(RoiDto {
        method: method.into(),
        denomination: denom.into(),
        since: start.0,
        start_value: s.to_string(),
        current_value: c.to_string(),
        pct_change: pct.to_string(),
        days_elapsed: days,
    })
}

/// Lifetime ROI: signed sum of `usd_value_at_event` across all
/// `capital_events` (deposits +, withdrawals −) compared against the
/// latest `balance_snapshots.total_usd`. Capital events with NULL
/// `usd_value_at_event` are skipped — the operator can fill them in
/// later for a more accurate basis.
pub async fn lifetime(state: &AppStateInner) -> Result<LifetimeRoiDto> {
    let mut conn = db::checkout(&state.pool).await?;

    let rows: Vec<(String, Option<Decimal>, DateTime<Utc>)> = capital_events::table
        .filter(capital_events::usd_value_at_event.is_not_null())
        .select((
            capital_events::direction,
            capital_events::usd_value_at_event,
            capital_events::occurred_at,
        ))
        .load(&mut *conn)
        .await?;

    let mut deployed = Decimal::ZERO;
    let mut since: Option<DateTime<Utc>> = None;
    for (direction, usd, at) in &rows {
        let Some(usd) = usd else { continue };
        match direction.as_str() {
            "deposit" => deployed += *usd,
            "withdraw" => deployed -= *usd,
            _ => continue,
        }
        since = Some(match since {
            Some(s) if s <= *at => s,
            _ => *at,
        });
    }
    let event_count = rows.len() as i32;

    let current_usd: Decimal = balance_snapshots::table
        .select(balance_snapshots::total_usd)
        .order(balance_snapshots::taken_at.desc())
        .first::<Decimal>(&mut *conn)
        .await
        .optional()?
        .unwrap_or(Decimal::ZERO);

    let pnl = current_usd - deployed;
    let roi_pct = if !deployed.is_zero() {
        Some(
            (pnl / deployed * Decimal::from(100))
                .round_dp(2)
                .to_string(),
        )
    } else {
        None
    };

    // Drop the connection before calling attribution — it checks out its own.
    drop(conn);

    // Decompose pnl into market (HODL) vs trade (swap spread captured)
    // by running the full-history attribution. The chart already implements
    // exactly this decomposition between successive snapshots.
    let (market_pnl_usd, trade_pnl_usd) =
        match crate::server::api::charts::attribution(state, "all").await {
            Ok(a) => (
                a.market_pnl_usd
                    .parse::<Decimal>()
                    .ok()
                    .map(|d| d.round_dp(2).to_string()),
                a.trade_pnl_usd
                    .parse::<Decimal>()
                    .ok()
                    .map(|d| d.round_dp(2).to_string()),
            ),
            Err(e) => {
                tracing::warn!(error = %e, "lifetime ROI attribution unavailable");
                (None, None)
            }
        };

    Ok(LifetimeRoiDto {
        capital_deployed_usd: deployed.round_dp(2).to_string(),
        current_value_usd: current_usd.round_dp(2).to_string(),
        pnl_usd: pnl.round_dp(2).to_string(),
        roi_pct,
        since,
        event_count,
        market_pnl_usd,
        trade_pnl_usd,
    })
}
