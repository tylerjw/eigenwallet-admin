use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use diesel::sql_types::Timestamptz;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::schema::{balance_snapshots, swaps};
use crate::server::state::AppStateInner;
use crate::types::{ChartPoint, ChartSeries};

pub async fn account_value(
    state: &AppStateInner,
    period: &str,
    denom: &str,
) -> Result<ChartSeries> {
    let since = parse_period(period);
    let mut conn = db::checkout(&state.pool).await?;

    let rows: Vec<(DateTime<Utc>, Decimal, Decimal)> = balance_snapshots::table
        .filter(balance_snapshots::taken_at.ge(since))
        .select((
            balance_snapshots::taken_at,
            balance_snapshots::total_usd,
            balance_snapshots::total_btc,
        ))
        .order(balance_snapshots::taken_at.asc())
        .load(&mut *conn)
        .await?;

    let points = rows
        .into_iter()
        .map(|(t, usd, btc)| ChartPoint {
            t,
            v: match denom {
                "btc" => btc.to_string(),
                _ => usd.to_string(),
            },
        })
        .collect();

    Ok(ChartSeries {
        points,
        denomination: denom.to_string(),
        period: period.to_string(),
    })
}

pub async fn swap_count(state: &AppStateInner, period: &str) -> Result<ChartSeries> {
    let since = parse_period(period);
    let mut conn = db::checkout(&state.pool).await?;

    #[derive(QueryableByName, Debug)]
    struct DayBucket {
        #[diesel(sql_type = Timestamptz)]
        day: DateTime<Utc>,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }

    let rows: Vec<DayBucket> = diesel::sql_query(
        "SELECT date_trunc('day', started_at) AS day, COUNT(*)::bigint AS n \
         FROM swaps \
         WHERE started_at >= $1 \
         GROUP BY day ORDER BY day",
    )
    .bind::<Timestamptz, _>(since)
    .load(&mut *conn)
    .await?;

    let _ = swaps::table;
    let points = rows
        .into_iter()
        .map(|b| ChartPoint {
            t: b.day,
            v: b.n.to_string(),
        })
        .collect();

    Ok(ChartSeries {
        points,
        denomination: "count".to_string(),
        period: period.to_string(),
    })
}

fn parse_period(p: &str) -> DateTime<Utc> {
    let dur = match p {
        "24h" => ChronoDuration::hours(24),
        "7d" => ChronoDuration::days(7),
        "30d" => ChronoDuration::days(30),
        "90d" => ChronoDuration::days(90),
        "all" => ChronoDuration::days(3650),
        _ => ChronoDuration::days(7),
    };
    Utc::now() - dur
}
