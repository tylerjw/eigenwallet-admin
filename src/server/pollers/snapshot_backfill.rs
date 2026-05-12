//! One-shot historical `balance_snapshots` reconstruction.
//!
//! The live `balance_snapshot` poller only writes a row every 5 minutes from
//! the moment this pod first booted. The /charts and Overview value charts
//! therefore start abruptly when the admin pod was first deployed. With the
//! Kraken hourly OHLC backfill populating `cex_prices` back to the earliest
//! swap, we can reconstruct holdings backwards by walking `swaps` and
//! `capital_events` from "now" toward the past, and value each hour against
//! the hourly OHLC sample. We then write one hourly `balance_snapshots` row
//! per hour from earliest-event to now (skipping any hour where the live
//! poller already wrote a row — `ON CONFLICT (taken_at) DO NOTHING`).
//!
//! Runs ONCE at startup, AFTER `kraken_backfill::run_once` finishes, so we
//! see the freshly-populated hourly OHLC samples.
//!
//! ## Approximation caveats
//! - Refunds and punishments are treated heuristically (see [`rewind_swap`]).
//!   For a v1 chart, an occasional off-by-one swap is invisible.
//! - We snap event timestamps DOWN to the hour. Multiple events within the
//!   same hour all roll into the same bucket, with the holdings as-of the
//!   LAST event in that hour (i.e. closest to the end of the hour).

use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::models::{BalanceSnapshot, CapitalEvent, CexPrice, Swap};
use crate::server::schema::{balance_snapshots, capital_events, cex_prices, swaps};
use crate::server::state::AppState;

/// How many existing rows below the earliest-event hour we'll tolerate before
/// concluding "backfill already ran, skip". The live poller writes once every
/// 5 minutes after pod boot — historical hours should have zero rows from it.
const ALREADY_BACKFILLED_THRESHOLD: i64 = 100;

/// Max temporal distance from a target hour to a `cex_prices` sample before
/// we give up valuing that hour. Kraken OHLC is hourly, so a 1-hour window
/// catches both the on-hour sample and the next-hour sample if the hour is
/// missing for some reason.
const PRICE_LOOKUP_WINDOW_SECS: i64 = 3600;

pub async fn run_once(state: AppState) -> Result<()> {
    let mut conn = db::checkout(&state.pool).await?;

    // 1. Pull every completed swap (DESC by completed_at) and every capital
    //    event (DESC by occurred_at). These are the holdings-altering events
    //    we walk backwards across. `swaps` without completed_at are still
    //    in-flight; they haven't moved final holdings yet.
    let swaps_rows: Vec<Swap> = swaps::table
        .filter(swaps::completed_at.is_not_null())
        .select(Swap::as_select())
        .order(swaps::completed_at.desc())
        .load(&mut *conn)
        .await?;

    let capital_rows: Vec<CapitalEvent> = capital_events::table
        .select(CapitalEvent::as_select())
        .order(capital_events::occurred_at.desc())
        .load(&mut *conn)
        .await?;

    // Build a single descending-timeline of events. Each entry is
    // (timestamp, EventKind).
    let mut timeline: Vec<(DateTime<Utc>, EventKind)> =
        Vec::with_capacity(swaps_rows.len() + capital_rows.len());
    for s in &swaps_rows {
        // `completed_at.is_not_null()` filter above guarantees Some.
        if let Some(t) = s.completed_at {
            timeline.push((t, EventKind::Swap(s.clone())));
        }
    }
    for c in &capital_rows {
        timeline.push((c.occurred_at, EventKind::Capital(c.clone())));
    }
    // Sort DESC by timestamp — walk from "now" toward the past.
    timeline.sort_by_key(|entry| std::cmp::Reverse(entry.0));

    let Some(earliest_event_at) = timeline.last().map(|(t, _)| *t) else {
        tracing::info!("snapshot-backfill: no swaps or capital events, skipping");
        return Ok(());
    };

    // 2. Idempotency: if there are already a lot of snapshot rows older than
    //    the earliest event + 1h, an earlier backfill run already filled them.
    let cutoff = earliest_event_at + chrono::Duration::hours(1);
    let preexisting: i64 = balance_snapshots::table
        .filter(balance_snapshots::taken_at.lt(cutoff))
        .count()
        .get_result(&mut *conn)
        .await?;
    if preexisting > ALREADY_BACKFILLED_THRESHOLD {
        tracing::info!(
            preexisting,
            "snapshot-backfill: {preexisting} rows already exist below earliest event, skipping"
        );
        return Ok(());
    }

    // 3. Pull every cex_prices row once for an in-memory hourly lookup. Even a
    //    year of hourly OHLC is < 10k rows.
    let prices: Vec<CexPrice> = cex_prices::table
        .order(cex_prices::sampled_at.asc())
        .load(&mut *conn)
        .await?;
    drop(conn);

    let hourly_prices = bucket_prices_by_hour(&prices);

    // 4. Get current wallet balances. These are the "after all events" state;
    //    we'll rewind from here. If the RPC fails, we can't do anything.
    let btc_now = state.asb.bitcoin_balance().await?;
    let xmr_now = state.asb.monero_balance().await?;
    let mut btc_sat: i64 = btc_now.balance;
    let xmr_atomic_decimal: Decimal = xmr_now.balance.to_string().parse().unwrap_or(Decimal::ZERO);
    let mut xmr_atomic: Decimal = xmr_atomic_decimal;

    // 5. Walk DESC across the timeline. For each event, the holdings AS OF
    //    that event's timestamp (rounded down to the hour) are the current
    //    `(btc_sat, xmr_atomic)`. We record into `holdings_by_hour`, then
    //    rewind to find the holdings PRIOR to that event.
    //
    //    When multiple events fall in the same hour, the first one we hit
    //    (the latest in time) wins — i.e. we record the AFTER-the-hour state
    //    once per hour, which is what the live poller would have captured at
    //    the top of the next hour anyway.
    let now_hour = floor_to_hour(Utc::now());
    let earliest_hour = floor_to_hour(earliest_event_at);

    // Map of hour -> (btc_sat, xmr_atomic). We record the post-event state at
    // event hour and rewind for the previous hour boundary.
    let mut holdings_by_hour: BTreeMap<DateTime<Utc>, (i64, Decimal)> = BTreeMap::new();

    // Seed every hour from `now` back to `earliest_hour` with the current
    // balances; we'll overwrite as we walk events backwards.
    let mut h = now_hour;
    while h >= earliest_hour {
        holdings_by_hour.insert(h, (btc_sat, xmr_atomic));
        h -= chrono::Duration::hours(1);
    }

    // Now walk events DESC and rewrite the prefix-of-hours <= event_hour
    // to the *pre-event* holdings.
    for (event_ts, kind) in &timeline {
        // Apply the rewind. After this, (btc_sat, xmr_atomic) reflect the
        // holdings BEFORE this event was applied.
        match kind {
            EventKind::Swap(s) => {
                let (b, x) = rewind_swap(s, btc_sat, xmr_atomic);
                btc_sat = b;
                xmr_atomic = x;
            }
            EventKind::Capital(c) => {
                let (b, x) = rewind_capital(c, btc_sat, xmr_atomic);
                btc_sat = b;
                xmr_atomic = x;
            }
        }
        // Every hour strictly EARLIER than the event hour now reflects the
        // pre-event holdings. The event-hour itself keeps the post-event
        // state we already wrote when seeding (or from a later-in-time event
        // already processed).
        let event_hour = floor_to_hour(*event_ts);
        let mut hour = event_hour - chrono::Duration::hours(1);
        while hour >= earliest_hour {
            holdings_by_hour.insert(hour, (btc_sat, xmr_atomic));
            hour -= chrono::Duration::hours(1);
        }
    }

    // 6. Build one BalanceSnapshot per hour, valued against the nearest
    //    hourly price sample. Skip hours with no price within the window or
    //    with zero/missing BTC/XMR USD prices (avoid div-by-zero downstream).
    let mut rows: Vec<BalanceSnapshot> = Vec::with_capacity(holdings_by_hour.len());
    for (hour, (btc, xmr)) in &holdings_by_hour {
        let Some(price) = nearest_hourly_price(&hourly_prices, *hour) else {
            continue;
        };
        let btc_usd = price.btc_usd.unwrap_or(Decimal::ZERO);
        let xmr_usd = price.xmr_usd.unwrap_or(Decimal::ZERO);
        if btc_usd.is_zero() || xmr_usd.is_zero() {
            // Can't compute total_btc without a non-zero btc_usd, and a row
            // with zero prices is misleading on the chart.
            continue;
        }
        let btc_decimal = Decimal::from(*btc) / Decimal::from(100_000_000i64);
        let xmr_decimal = *xmr / Decimal::from(1_000_000_000_000i64);
        let total_usd = btc_decimal * btc_usd + xmr_decimal * xmr_usd;
        let total_btc = btc_decimal + (xmr_decimal * xmr_usd / btc_usd);
        rows.push(BalanceSnapshot {
            taken_at: *hour,
            btc_sat: *btc,
            xmr_atomic: *xmr,
            btc_usd,
            xmr_usd,
            total_usd,
            total_btc,
        });
    }

    if rows.is_empty() {
        tracing::info!(
            "snapshot-backfill: no hours valued (cex_prices likely empty), nothing inserted"
        );
        return Ok(());
    }

    // 7. Bulk-insert with conflict ignore so we never clobber the live poller.
    let mut conn = db::checkout(&state.pool).await?;
    let mut inserted: usize = 0;
    for chunk in rows.chunks(500) {
        let n = diesel::insert_into(balance_snapshots::table)
            .values(chunk)
            .on_conflict(balance_snapshots::taken_at)
            .do_nothing()
            .execute(&mut *conn)
            .await?;
        inserted += n;
    }

    tracing::info!(
        inserted,
        attempted = rows.len(),
        from = %earliest_hour,
        to = %now_hour,
        "snapshot-backfill: inserted {inserted}/{} hourly balance snapshots",
        rows.len()
    );

    Ok(())
}

enum EventKind {
    Swap(Swap),
    Capital(CapitalEvent),
}

/// Compute the holdings BEFORE this swap completed, given the holdings AFTER.
///
/// Maker semantics:
/// - "redeemed": maker received `btc_sat` and paid `xmr_atomic`. So before the
///   swap, btc was lower by btc_sat, xmr was higher by xmr_atomic.
/// - "refunded": BTC returned to the taker. Wallet's BTC didn't change net;
///   approximate as `(btc + btc_sat, xmr)` (BTC was briefly locked then went
///   back) — coarse, but correct in aggregate for v1.
/// - "punished": maker kept both sides. Treat the same as redeemed.
fn rewind_swap(s: &Swap, btc_after: i64, xmr_after: Decimal) -> (i64, Decimal) {
    let state = s.state.to_ascii_lowercase();
    if state.contains("redeemed") || state.contains("punished") {
        (btc_after - s.btc_sat, xmr_after + s.xmr_atomic)
    } else if state.contains("refunded") {
        (btc_after + s.btc_sat, xmr_after)
    } else {
        // Unknown completed state — leave holdings unchanged.
        (btc_after, xmr_after)
    }
}

/// Compute the holdings BEFORE this capital event, given the holdings AFTER.
fn rewind_capital(c: &CapitalEvent, btc_after: i64, xmr_after: Decimal) -> (i64, Decimal) {
    let asset = c.asset.to_ascii_uppercase();
    let direction = c.direction.to_ascii_lowercase();
    match asset.as_str() {
        "BTC" => {
            // Convert Decimal amount_atomic to i64 satoshi. Capital event
            // amounts for BTC are stored in satoshi (see migrations).
            let amount_sat = decimal_to_i64(c.amount_atomic);
            match direction.as_str() {
                "deposit" => (btc_after - amount_sat, xmr_after),
                "withdraw" => (btc_after + amount_sat, xmr_after),
                _ => (btc_after, xmr_after),
            }
        }
        "XMR" => match direction.as_str() {
            "deposit" => (btc_after, xmr_after - c.amount_atomic),
            "withdraw" => (btc_after, xmr_after + c.amount_atomic),
            _ => (btc_after, xmr_after),
        },
        // USD events represent fiat into Kraken, not wallet movement.
        _ => (btc_after, xmr_after),
    }
}

fn decimal_to_i64(d: Decimal) -> i64 {
    // Truncate toward zero; capital_events amounts are integer-valued in
    // atomic units, so this is a safe round-trip.
    d.trunc().to_string().parse().unwrap_or(0)
}

/// Snap a timestamp DOWN to the start of its hour (UTC). Uses chrono's
/// `DurationRound::duration_trunc`, which is infallible for hour windows on
/// any timezone-naive UTC value.
fn floor_to_hour(t: DateTime<Utc>) -> DateTime<Utc> {
    t.duration_trunc(TimeDelta::hours(1)).unwrap_or(t)
}

/// Bucket the price-table rows by their hour (UTC). When multiple samples
/// fall in the same hour (e.g. minute-cadence live poller + hourly backfill),
/// keep the LAST one — it's the most recent observation for that hour.
fn bucket_prices_by_hour(prices: &[CexPrice]) -> BTreeMap<DateTime<Utc>, CexPrice> {
    let mut out: BTreeMap<DateTime<Utc>, CexPrice> = BTreeMap::new();
    for p in prices {
        out.insert(floor_to_hour(p.sampled_at), p.clone());
    }
    out
}

/// Find the closest-by-hour price sample to `at`, within
/// `PRICE_LOOKUP_WINDOW_SECS`. Walks +/- 1 hour around the target since the
/// price table is hour-bucketed.
fn nearest_hourly_price(
    by_hour: &BTreeMap<DateTime<Utc>, CexPrice>,
    at: DateTime<Utc>,
) -> Option<&CexPrice> {
    let target = floor_to_hour(at);
    if let Some(p) = by_hour.get(&target) {
        return Some(p);
    }
    // Fall back to the neighboring hours.
    let one_hour = chrono::Duration::seconds(PRICE_LOOKUP_WINDOW_SECS);
    let prev = target - one_hour;
    let next = target + one_hour;
    let mut best: Option<&CexPrice> = None;
    let mut best_gap = i64::MAX;
    for candidate in [&prev, &next] {
        if let Some(p) = by_hour.get(candidate) {
            let gap = (p.sampled_at - at).num_seconds().abs();
            if gap < best_gap {
                best_gap = gap;
                best = Some(p);
            }
        }
    }
    best
}
