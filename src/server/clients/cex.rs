//! Kraken + KuCoin REST tickers. Cached in-memory.

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CexSnapshot {
    pub btc_usd: Option<Decimal>,
    pub xmr_usd: Option<Decimal>,
    pub btc_xmr: Option<Decimal>,
    pub sources: Vec<String>,
    pub sampled_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Default)]
pub struct CexCache {
    pub last: Option<CexSnapshot>,
    pub fetched_at: Option<Instant>,
}

impl CexCache {
    pub fn is_fresh(&self, ttl: Duration) -> bool {
        self.fetched_at.map(|t| t.elapsed() < ttl).unwrap_or(false)
    }
}

pub async fn fetch_all(client: &Client) -> CexSnapshot {
    let kraken = fetch_kraken(client).await;
    let kucoin = fetch_kucoin(client).await;

    let mut sources = Vec::new();
    let mut btc_usd_samples: Vec<Decimal> = Vec::new();
    let mut xmr_usd_samples: Vec<Decimal> = Vec::new();
    let mut btc_xmr_samples: Vec<Decimal> = Vec::new();

    if let Ok((bu, xu, bx)) = kraken {
        sources.push("kraken".to_string());
        if let Some(v) = bu {
            btc_usd_samples.push(v);
        }
        if let Some(v) = xu {
            xmr_usd_samples.push(v);
        }
        if let Some(v) = bx {
            btc_xmr_samples.push(v);
        }
    }
    if let Ok((bu, xu)) = kucoin {
        sources.push("kucoin".to_string());
        if let Some(v) = bu {
            btc_usd_samples.push(v);
        }
        if let Some(v) = xu {
            xmr_usd_samples.push(v);
        }
    }

    let btc_usd = median(&btc_usd_samples);
    let xmr_usd = median(&xmr_usd_samples);
    let btc_xmr = if let Some(b) = btc_xmr_samples.first().copied() {
        Some(b)
    } else if let (Some(bu), Some(xu)) = (btc_usd, xmr_usd) {
        if xu.is_zero() { None } else { Some(xu / bu) }
    } else {
        None
    };

    CexSnapshot {
        btc_usd,
        xmr_usd,
        btc_xmr,
        sources,
        sampled_at: chrono::Utc::now(),
    }
}

fn median(values: &[Decimal]) -> Option<Decimal> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort();
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[mid]
    } else {
        (v[mid - 1] + v[mid]) / Decimal::from(2)
    })
}

async fn fetch_kraken(
    client: &Client,
) -> Result<(Option<Decimal>, Option<Decimal>, Option<Decimal>)> {
    let resp: serde_json::Value = client
        .get("https://api.kraken.com/0/public/Ticker?pair=XBTUSD,XMRUSD,XMRXBT")
        .send()
        .await?
        .json()
        .await?;
    let result = resp
        .get("result")
        .ok_or_else(|| anyhow!("kraken: no result"))?;
    let btc_usd = first_close(result, &["XXBTZUSD", "XBTUSD"]);
    let xmr_usd = first_close(result, &["XXMRZUSD", "XMRUSD"]);
    let xmr_btc = first_close(result, &["XXMRXXBT", "XMRXBT"]);
    Ok((btc_usd, xmr_usd, xmr_btc))
}

fn first_close(result: &serde_json::Value, keys: &[&str]) -> Option<Decimal> {
    for k in keys {
        if let Some(o) = result.get(*k)
            && let Some(c) = o
                .get("c")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
            && let Some(s) = c.as_str()
            && let Ok(d) = s.parse::<Decimal>()
        {
            return Some(d);
        }
    }
    None
}

async fn fetch_kucoin(client: &Client) -> Result<(Option<Decimal>, Option<Decimal>)> {
    let btc = fetch_kucoin_one(client, "BTC-USDT").await.ok();
    let xmr = fetch_kucoin_one(client, "XMR-USDT").await.ok();
    Ok((btc, xmr))
}

async fn fetch_kucoin_one(client: &Client, symbol: &str) -> Result<Decimal> {
    let url = format!("https://api.kucoin.com/api/v1/market/orderbook/level1?symbol={symbol}");
    let resp: serde_json::Value = client.get(url).send().await?.json().await?;
    let price = resp
        .get("data")
        .and_then(|d| d.get("price"))
        .and_then(|p| p.as_str())
        .ok_or_else(|| anyhow!("kucoin: no data.price for {symbol}"))?;
    price
        .parse::<Decimal>()
        .map_err(|e| anyhow!("kucoin parse: {e}"))
}
