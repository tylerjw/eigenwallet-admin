use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::schema::balance_snapshots;
use crate::server::state::AppStateInner;
use crate::types::RoiDto;

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
