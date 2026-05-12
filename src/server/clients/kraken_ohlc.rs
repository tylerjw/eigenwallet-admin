//! Kraken public OHLC endpoint client. Used by the one-shot backfill poller
//! to seed `cex_prices` with hourly history that predates the admin pod
//! recording live samples.
//!
//! Endpoint shape (public, no auth):
//!   GET https://api.kraken.com/0/public/OHLC?pair=<PAIR>&interval=60&since=<ts>
//! Response:
//!   {"error": [...], "result": {"<KEY>": [[time, open, high, low, close, vwap,
//!                                          volume, count], ...],
//!                                "last": <unix_ts>}}
//!
//! Kraken's pair-key naming in the response differs from the request pair —
//! e.g. `XBTUSD` -> `XXBTZUSD`, `XMRUSD` -> `XXMRZUSD`, `XMRXBT` -> `XXMRXXBT`.
//! We accept whatever non-`last` key the response carries.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct OhlcCandle {
    pub time: DateTime<Utc>,
    pub close: Decimal,
}

/// Result of a single OHLC fetch.
#[derive(Debug, Clone)]
pub struct OhlcPage {
    pub candles: Vec<OhlcCandle>,
    /// Server-reported cursor (unix seconds). Used as `since` for the next page
    /// when paging through more than ~720 candles.
    pub last: i64,
}

/// Fetch up to ~720 hourly candles for `pair` starting at `since`.
///
/// `pair` should be the request-form symbol (`XBTUSD`, `XMRUSD`, `XMRXBT`).
pub async fn fetch_ohlc(client: &Client, pair: &str, since: DateTime<Utc>) -> Result<OhlcPage> {
    let url = format!(
        "https://api.kraken.com/0/public/OHLC?pair={pair}&interval=60&since={}",
        since.timestamp()
    );
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;

    if let Some(errs) = resp.get("error").and_then(|e| e.as_array())
        && !errs.is_empty()
    {
        return Err(anyhow!("kraken ohlc error: {errs:?}"));
    }

    let result = resp
        .get("result")
        .and_then(|r| r.as_object())
        .ok_or_else(|| anyhow!("kraken ohlc: missing result for {pair}"))?;

    let last = result
        .get("last")
        .and_then(|l| l.as_i64())
        .ok_or_else(|| anyhow!("kraken ohlc: missing result.last for {pair}"))?;

    // Find the series key (anything that's not "last"). Usually exactly one.
    let series = result
        .iter()
        .find(|(k, _)| k.as_str() != "last")
        .map(|(_, v)| v)
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("kraken ohlc: no series array for {pair}"))?;

    let mut candles = Vec::with_capacity(series.len());
    for row in series {
        let arr = match row.as_array() {
            Some(a) if a.len() >= 5 => a,
            _ => continue,
        };
        let ts = match arr[0].as_i64() {
            Some(t) => t,
            None => continue,
        };
        // close is index 4, encoded as a string
        let close_str = match arr[4].as_str() {
            Some(s) => s,
            None => continue,
        };
        let close = match close_str.parse::<Decimal>() {
            Ok(d) => d,
            Err(_) => continue,
        };
        let Some(time) = DateTime::<Utc>::from_timestamp(ts, 0) else {
            continue;
        };
        candles.push(OhlcCandle { time, close });
    }

    Ok(OhlcPage { candles, last })
}
