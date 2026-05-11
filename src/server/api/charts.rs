use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use diesel::sql_types::Timestamptz;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::schema::{balance_snapshots, capital_events, swaps};
use crate::server::state::AppStateInner;
use crate::types::{AttributionDto, ChartPoint, ChartSeries};

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

/// P&L attribution between consecutive balance snapshots. For each step:
///   price_step  = btc(i) × Δbtc_usd + xmr(i) × Δxmr_usd
///   trade_step  = Δbtc × btc_usd(i+1) + Δxmr × xmr_usd(i+1) - capital_in_step
///   capital_in_step = sum of deposit USD values minus withdrawal USD values
///                     for capital_events with occurred_at in (t(i), t(i+1)].
///
/// Cumulative sums give the three series. The "no-trade baseline" at time t
/// is start_value + cumulative price_step + cumulative capital_in_step — what
/// the portfolio would be worth if no swaps had happened.
pub async fn attribution(state: &AppStateInner, period: &str) -> Result<AttributionDto> {
    let since = parse_period(period);
    let mut conn = db::checkout(&state.pool).await?;

    type SnapshotRow = (DateTime<Utc>, i64, Decimal, Decimal, Decimal, Decimal);
    let snapshots: Vec<SnapshotRow> = balance_snapshots::table
        .filter(balance_snapshots::taken_at.ge(since))
        .select((
            balance_snapshots::taken_at,
            balance_snapshots::btc_sat,
            balance_snapshots::xmr_atomic,
            balance_snapshots::btc_usd,
            balance_snapshots::xmr_usd,
            balance_snapshots::total_usd,
        ))
        .order(balance_snapshots::taken_at.asc())
        .load(&mut *conn)
        .await?;

    type CapitalRow = (DateTime<Utc>, String, Option<Decimal>);
    let cap_events: Vec<CapitalRow> = capital_events::table
        .filter(capital_events::occurred_at.ge(since))
        .select((
            capital_events::occurred_at,
            capital_events::direction,
            capital_events::usd_value_at_event,
        ))
        .order(capital_events::occurred_at.asc())
        .load(&mut *conn)
        .await?;

    let zero = Decimal::ZERO;
    let sats_per_btc = Decimal::from(100_000_000i64);
    let pico_per_xmr = Decimal::from(1_000_000_000_000i64);

    let mut actual = Vec::with_capacity(snapshots.len());
    let mut baseline = Vec::with_capacity(snapshots.len());

    if snapshots.is_empty() {
        return Ok(AttributionDto {
            actual,
            no_trade_baseline: baseline,
            start_value_usd: "0".into(),
            end_value_usd: "0".into(),
            market_pnl_usd: "0".into(),
            trade_pnl_usd: "0".into(),
            capital_flow_usd: "0".into(),
            period: period.to_string(),
            sample_count: 0,
        });
    }

    let start_value = snapshots[0].5;
    let mut cum_market = zero;
    let mut cum_capital = zero;

    // First point: start of period — by definition no PnL has accumulated yet.
    actual.push(ChartPoint {
        t: snapshots[0].0,
        v: start_value.to_string(),
    });
    baseline.push(ChartPoint {
        t: snapshots[0].0,
        v: start_value.to_string(),
    });

    for i in 1..snapshots.len() {
        let prev = &snapshots[i - 1];
        let cur = &snapshots[i];
        let btc_prev = Decimal::from(prev.1) / sats_per_btc;
        let xmr_prev = prev.2 / pico_per_xmr;
        let market_step = btc_prev * (cur.3 - prev.3) + xmr_prev * (cur.4 - prev.4);
        cum_market += market_step;

        // Sum capital events strictly between prev.0 and cur.0.
        let cap_step: Decimal = cap_events
            .iter()
            .filter(|(t, _, _)| *t > prev.0 && *t <= cur.0)
            .map(|(_, dir, usd)| {
                let v = usd.unwrap_or(zero);
                if dir == "deposit" { v } else { -v }
            })
            .sum();
        cum_capital += cap_step;

        actual.push(ChartPoint {
            t: cur.0,
            v: cur.5.to_string(),
        });
        baseline.push(ChartPoint {
            t: cur.0,
            v: (start_value + cum_market + cum_capital).to_string(),
        });
    }

    let end_value = snapshots.last().map(|s| s.5).unwrap_or(zero);
    let trade_pnl = end_value - start_value - cum_market - cum_capital;

    Ok(AttributionDto {
        actual,
        no_trade_baseline: baseline,
        start_value_usd: start_value.to_string(),
        end_value_usd: end_value.to_string(),
        market_pnl_usd: cum_market.to_string(),
        trade_pnl_usd: trade_pnl.to_string(),
        capital_flow_usd: cum_capital.to_string(),
        period: period.to_string(),
        sample_count: snapshots.len() as i32,
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
