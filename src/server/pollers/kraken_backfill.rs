//! One-shot Kraken hourly OHLC backfill into `cex_prices`.
//!
//! Many swaps in the DB completed weeks before this pod started recording live
//! CEX samples, which makes `compute_profit_usd`'s nearest-sample lookup snap
//! every ancient swap to the same stale price. To fix that we backfill hourly
//! Kraken OHLC for the period from the oldest swap's `started_at` up to now,
//! one row per hour with `sources = ["kraken-ohlc-backfill"]`. The live poller
//! continues writing minute-cadence samples in parallel; `ON CONFLICT
//! DO NOTHING` keeps the two streams from clobbering each other.
//!
//! Spawned ONCE at startup (not on an interval). Cheap to re-run: an early
//! short-circuit avoids hammering Kraken once we're already filled in.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use diesel::prelude::*;
use diesel::sql_types::BigInt;
use diesel_async::RunQueryDsl;
use reqwest::Client;
use rust_decimal::Decimal;

use crate::server::clients::kraken_ohlc::{self, OhlcCandle};
use crate::server::db;
use crate::server::models::CexPrice;
use crate::server::schema::{cex_prices, swaps};
use crate::server::state::AppState;

const BACKFILL_SOURCE: &str = "kraken-ohlc-backfill";
const PAIRS: [&str; 3] = ["XBTUSD", "XMRUSD", "XMRXBT"];
/// Kraken's documented rate-limit budget is generous on public endpoints, but
/// we only need ~6 calls total. Pause between calls to stay polite.
const INTER_CALL_DELAY: Duration = Duration::from_millis(1500);
/// Each call yields ~720 hourly candles. Cap total pages per pair as a guard
/// against a runaway pagination loop (e.g. server returning stuck `last`).
const MAX_PAGES_PER_PAIR: usize = 12;

pub async fn run_once(state: AppState) -> Result<()> {
    let mut conn = db::checkout(&state.pool).await?;

    // 1. Find the earliest swap. If there are no swaps yet, nothing to backfill.
    let earliest: Option<DateTime<Utc>> = swaps::table
        .select(swaps::started_at)
        .order(swaps::started_at.asc())
        .first(&mut *conn)
        .await
        .optional()?;
    let Some(earliest) = earliest else {
        tracing::info!("kraken-ohlc backfill: no swaps in DB, skipping");
        return Ok(());
    };

    let now = Utc::now();
    let days = (now - earliest).num_days().max(1);

    // 2. Cheap idempotency gate. If we already have ~80% of the hours covered
    //    by a previous backfill run, skip the network calls entirely.
    let existing: i64 = diesel::sql_query(
        "SELECT COUNT(*)::bigint AS count FROM cex_prices \
         WHERE sources @> ARRAY['kraken-ohlc-backfill']::text[]",
    )
    .get_result::<BackfillCount>(&mut *conn)
    .await
    .map(|c| c.count)
    .unwrap_or(0);

    let expected = (24 * days) as f64 * 0.8;
    if (existing as f64) > expected {
        tracing::info!(
            existing,
            days,
            "kraken-ohlc backfill: already covered ({existing} rows), skipping"
        );
        return Ok(());
    }

    // Drop the DB connection while we make network calls; we'll re-acquire to
    // insert. Pool has limited slots — don't hold one across slow IO.
    drop(conn);

    let http = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| anyhow::anyhow!("kraken-ohlc http client: {e}"))?;

    // 3. Pull hourly closes for each pair.
    let mut series: [Vec<OhlcCandle>; 3] = Default::default();
    for (idx, pair) in PAIRS.iter().enumerate() {
        series[idx] = fetch_pair(&http, pair, earliest).await?;
        tracing::info!(
            pair,
            candles = series[idx].len(),
            "kraken-ohlc backfill: pulled series"
        );
    }
    let [btc_usd_series, xmr_usd_series, xmr_btc_series] = series;

    // 4. Hour-align and union into one sorted timeline.
    let btc_usd_map = candles_by_hour(&btc_usd_series);
    let xmr_usd_map = candles_by_hour(&xmr_usd_series);
    let xmr_btc_map = candles_by_hour(&xmr_btc_series);

    let mut all_hours: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    all_hours.extend(btc_usd_map.keys().copied());
    all_hours.extend(xmr_usd_map.keys().copied());
    all_hours.extend(xmr_btc_map.keys().copied());

    let rows: Vec<CexPrice> = all_hours
        .iter()
        .filter_map(|&hour_ts| {
            let sampled_at = Utc.timestamp_opt(hour_ts, 0).single()?;
            let btc_usd = btc_usd_map.get(&hour_ts).copied();
            let xmr_usd = xmr_usd_map.get(&hour_ts).copied();
            // Pair quote is XMR/BTC (price of 1 XMR in BTC); `cex_prices.btc_xmr`
            // follows the same convention (see clients/cex.rs).
            let btc_xmr = xmr_btc_map.get(&hour_ts).copied();
            Some(CexPrice {
                sampled_at,
                btc_usd,
                xmr_usd,
                btc_xmr,
                sources: vec![BACKFILL_SOURCE.to_string()],
            })
        })
        .collect();

    if rows.is_empty() {
        tracing::info!("kraken-ohlc backfill: no candles returned, nothing to insert");
        return Ok(());
    }

    // Note: `<[_]>::first` / `<[_]>::last` — disambiguate from diesel's
    // `FirstDsl::first`, which the trait imports otherwise dispatch to.
    let period_start = <[CexPrice]>::first(&rows).map(|r| r.sampled_at);
    let period_end = <[CexPrice]>::last(&rows).map(|r| r.sampled_at);

    // 5. Insert in chunks; postgres parameter limit is 65k and we have 5 cols
    //    per row, so 1000-row chunks are safe.
    let mut conn = db::checkout(&state.pool).await?;
    let mut inserted: usize = 0;
    for chunk in rows.chunks(1000) {
        let n = diesel::insert_into(cex_prices::table)
            .values(chunk)
            .on_conflict(cex_prices::sampled_at)
            .do_nothing()
            .execute(&mut *conn)
            .await?;
        inserted += n;
    }

    tracing::info!(
        inserted,
        ?period_start,
        ?period_end,
        "kraken-ohlc backfill inserted {inserted} rows over period {period_start:?}..{period_end:?}"
    );

    Ok(())
}

#[derive(diesel::QueryableByName)]
struct BackfillCount {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

/// Page through Kraken OHLC for one pair until we reach "now" or stop making
/// forward progress.
async fn fetch_pair(client: &Client, pair: &str, start: DateTime<Utc>) -> Result<Vec<OhlcCandle>> {
    let mut out: Vec<OhlcCandle> = Vec::new();
    let mut cursor = start;
    for page_idx in 0..MAX_PAGES_PER_PAIR {
        if page_idx > 0 {
            tokio::time::sleep(INTER_CALL_DELAY).await;
        }
        let page = kraken_ohlc::fetch_ohlc(client, pair, cursor).await?;
        let returned = page.candles.len();
        out.extend(page.candles);

        let next_cursor = match DateTime::<Utc>::from_timestamp(page.last, 0) {
            Some(t) => t,
            None => break,
        };
        // Stop if the cursor didn't advance (no more data) or we're caught up.
        if next_cursor <= cursor || returned == 0 {
            break;
        }
        cursor = next_cursor;
        if cursor >= Utc::now() {
            break;
        }
    }
    Ok(out)
}

/// Bucket candles by their unix-second hour (already hour-aligned from Kraken
/// at interval=60). Returned map is keyed by unix seconds for cheap union.
fn candles_by_hour(candles: &[OhlcCandle]) -> BTreeMap<i64, Decimal> {
    let mut out = BTreeMap::new();
    for c in candles {
        // Snap to the hour just in case Kraken ever returns sub-hour offsets.
        let ts = c.time.timestamp();
        let hour_ts = ts - (ts.rem_euclid(3600));
        out.insert(hour_ts, c.close);
    }
    out
}
