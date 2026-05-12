use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use diesel::sql_types::Timestamptz;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::schema::{balance_snapshots, capital_events, swaps};
use crate::server::state::AppStateInner;
use crate::types::{AttributionDto, CapitalEventMarker, ChartPoint, ChartSeries, OverviewChartDto};

pub async fn account_value(
    state: &AppStateInner,
    period: &str,
    denom: &str,
) -> Result<ChartSeries> {
    let since = parse_period(period);
    let mut conn = db::checkout(&state.pool).await?;

    let rows: Vec<(DateTime<Utc>, Decimal, Decimal)> = balance_snapshots::table
        .filter(balance_snapshots::taken_at.ge(since))
        // Skip rows recorded before CEX prices populated; total_usd would be
        // zero on those and ruins the y-axis scale.
        .filter(balance_snapshots::total_usd.gt(Decimal::ZERO))
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
    let snapshots_raw: Vec<SnapshotRow> = balance_snapshots::table
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
    // Drop ANY snapshot with zero prices (CEX cache miss). These produce
    // garbage attribution: a row with btc_usd=0 next to one with btc_usd=80k
    // generates a market-PnL step of `holdings × 80k`, which dwarfs the real
    // signal. Leading and mid-series zero rows alike must be filtered.
    let snapshots: Vec<SnapshotRow> = snapshots_raw
        .into_iter()
        .filter(|r| !r.3.is_zero() && !r.4.is_zero() && !r.5.is_zero())
        .collect();

    // Pull asset + amount so we can fill in missing usd_value_at_event from
    // the nearest snapshot price. Without this, NULL-USD capital events
    // silently contribute zero to cum_capital and the trade-PnL residual
    // absorbs the missing capital — wrongly inflating "trades captured".
    type CapitalRow = (DateTime<Utc>, String, String, Decimal, Option<Decimal>);
    let cap_events: Vec<CapitalRow> = capital_events::table
        .filter(capital_events::occurred_at.ge(since))
        .select((
            capital_events::occurred_at,
            capital_events::direction,
            capital_events::asset,
            capital_events::amount_atomic,
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
    let mut market_cum = Vec::with_capacity(snapshots.len());
    let mut trade_cum = Vec::with_capacity(snapshots.len());
    let mut capital_cum = Vec::with_capacity(snapshots.len());

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
            market_cum,
            trade_cum,
            capital_cum,
            capital_events_missing_usd: 0,
            capital_events_total: cap_events.len() as i32,
        });
    }

    // Estimate USD value for capital events that have NULL `usd_value_at_event`
    // by linear-search for the nearest snapshot in time and pricing the amount
    // at that snapshot's btc_usd/xmr_usd. USD-denominated capital events are
    // already in USD so they don't need estimation.
    let nearest_snapshot_idx = |t: &DateTime<Utc>| -> usize {
        let mut best = (0usize, i64::MAX);
        for (i, s) in snapshots.iter().enumerate() {
            let d = (s.0 - *t).num_seconds().abs();
            if d < best.1 {
                best = (i, d);
            }
        }
        best.0
    };
    let mut missing_usd_count = 0i32;
    let cap_events_resolved: Vec<(DateTime<Utc>, String, Decimal)> = cap_events
        .iter()
        .map(|(t, dir, asset, amount_atomic, usd_opt)| {
            let usd = if let Some(v) = usd_opt {
                *v
            } else {
                missing_usd_count += 1;
                let snap = &snapshots[nearest_snapshot_idx(t)];
                match asset.as_str() {
                    "BTC" => (*amount_atomic / sats_per_btc) * snap.3,
                    "XMR" => (*amount_atomic / pico_per_xmr) * snap.4,
                    // USD asset: amount_atomic is the USD value itself (the
                    // capital.rs add() codepath only handles BTC/XMR today, but
                    // the schema allows USD; treat the atomic field as USD).
                    "USD" => *amount_atomic,
                    _ => zero,
                }
            };
            (*t, dir.clone(), usd)
        })
        .collect();
    if missing_usd_count > 0 {
        tracing::warn!(
            period = period,
            missing = missing_usd_count,
            total = cap_events.len(),
            "attribution: estimated USD value for {missing_usd_count} capital_events with NULL usd_value_at_event using nearest snapshot price",
        );
    }

    let start_value = snapshots[0].5;
    let mut cum_market = zero;
    let mut cum_capital = zero;

    // First point: start of period — by definition no PnL has accumulated yet.
    let t0 = snapshots[0].0;
    actual.push(ChartPoint {
        t: t0,
        v: start_value.to_string(),
    });
    baseline.push(ChartPoint {
        t: t0,
        v: start_value.to_string(),
    });
    market_cum.push(ChartPoint {
        t: t0,
        v: "0".into(),
    });
    trade_cum.push(ChartPoint {
        t: t0,
        v: "0".into(),
    });
    capital_cum.push(ChartPoint {
        t: t0,
        v: "0".into(),
    });

    for i in 1..snapshots.len() {
        let prev = &snapshots[i - 1];
        let cur = &snapshots[i];
        let btc_prev = Decimal::from(prev.1) / sats_per_btc;
        let xmr_prev = prev.2 / pico_per_xmr;
        let market_step = btc_prev * (cur.3 - prev.3) + xmr_prev * (cur.4 - prev.4);
        cum_market += market_step;

        // Sum capital events strictly between prev.0 and cur.0.
        let cap_step: Decimal = cap_events_resolved
            .iter()
            .filter(|(t, _, _)| *t > prev.0 && *t <= cur.0)
            .map(|(_, dir, usd)| if dir == "deposit" { *usd } else { -*usd })
            .sum();
        cum_capital += cap_step;

        // Trade-PnL as a running residual: end_so_far - start - cum_market - cum_capital
        let cur_trade_pnl = cur.5 - start_value - cum_market - cum_capital;

        actual.push(ChartPoint {
            t: cur.0,
            v: cur.5.to_string(),
        });
        baseline.push(ChartPoint {
            t: cur.0,
            v: (start_value + cum_market + cum_capital).to_string(),
        });
        market_cum.push(ChartPoint {
            t: cur.0,
            v: cum_market.to_string(),
        });
        trade_cum.push(ChartPoint {
            t: cur.0,
            v: cur_trade_pnl.to_string(),
        });
        capital_cum.push(ChartPoint {
            t: cur.0,
            v: cum_capital.to_string(),
        });
    }

    let end_value = snapshots.last().map(|s| s.5).unwrap_or(zero);
    let trade_pnl = end_value - start_value - cum_market - cum_capital;

    // Diagnostic log — helps explain unexpected "trade PnL" numbers when they
    // arise. Includes the identity check.
    tracing::debug!(
        period = period,
        snapshots = snapshots.len(),
        cap_events = cap_events.len(),
        missing_usd = missing_usd_count,
        start_value = %start_value,
        end_value = %end_value,
        cum_market = %cum_market,
        cum_capital = %cum_capital,
        trade_pnl = %trade_pnl,
        identity_check = %(end_value - start_value - cum_market - cum_capital - trade_pnl),
        "attribution computed",
    );

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
        market_cum,
        trade_cum,
        capital_cum,
        capital_events_missing_usd: missing_usd_count,
        capital_events_total: cap_events.len() as i32,
    })
}

/// Composite endpoint for the Overview chart tile. Bundles:
///   * the USD value series for `period`,
///   * the capital_events that fall inside `period` (deposit/withdraw markers),
///   * the trading-only delta (`trade_pnl_usd`) over the same window.
///
/// Attribution is best-effort: if it fails (e.g. <2 snapshots), the
/// `trade_only_delta_usd` is left as an empty string so the UI hides it.
pub async fn overview_chart(state: &AppStateInner, period: &str) -> Result<OverviewChartDto> {
    let series = account_value(state, period, "usd").await?;

    let since = parse_period(period);
    let mut conn = db::checkout(&state.pool).await?;

    type MarkerRow = (DateTime<Utc>, String, String, Option<Decimal>);
    let rows: Vec<MarkerRow> = capital_events::table
        .filter(capital_events::occurred_at.ge(since))
        .select((
            capital_events::occurred_at,
            capital_events::direction,
            capital_events::asset,
            capital_events::usd_value_at_event,
        ))
        .order(capital_events::occurred_at.asc())
        .load(&mut *conn)
        .await?;
    drop(conn);

    let markers = rows
        .into_iter()
        .map(|(at, direction, asset, usd_value)| CapitalEventMarker {
            at,
            direction,
            asset,
            usd_value: usd_value.map(|v| v.to_string()),
        })
        .collect();

    let trade_only_delta_usd = match attribution(state, period).await {
        Ok(a) if a.sample_count >= 2 => a.trade_pnl_usd,
        _ => String::new(),
    };

    Ok(OverviewChartDto {
        series,
        markers,
        trade_only_delta_usd,
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
