use anyhow::Result;
use rust_decimal::Decimal;

use crate::server::state::AppStateInner;
use crate::types::{ChartPoint, MarketPositionDto};

const SATS_PER_BTC: i64 = 100_000_000;

pub async fn position(state: &AppStateInner) -> Result<MarketPositionDto> {
    let quote = state.asb.get_current_quote().await.ok();
    let snap = state.cex.read().await.last.clone();
    let mid = snap.as_ref().and_then(|s| s.btc_xmr);

    let our_price = quote
        .as_ref()
        .map(|q| Decimal::from(q.price) / Decimal::from(SATS_PER_BTC));
    let our_spread = match (our_price, mid) {
        (Some(p), Some(m)) if !m.is_zero() => Some(((p - m) / m * Decimal::from(100)).round_dp(2)),
        _ => None,
    };

    let latest_scan = super::competitors::latest(state).await.ok().flatten();
    let mut competitor_spreads: Vec<Decimal> = Vec::new();
    if let Some(scan) = latest_scan.as_ref() {
        for q in &scan.quotes {
            if !q.reachable {
                continue;
            }
            if let Some(s) = &q.spread_vs_cex_pct
                && let Ok(d) = s.parse::<Decimal>()
            {
                competitor_spreads.push(d);
            }
        }
    }
    competitor_spreads.sort();
    let cheapest = competitor_spreads.first().copied();

    let our_rank =
        our_spread.map(|us| (competitor_spreads.iter().filter(|c| **c < us).count() as i32) + 1);
    let total = (competitor_spreads.len() as i32) + 1;

    Ok(MarketPositionDto {
        our_spread_pct: our_spread.map(|d| d.to_string()),
        our_price_btc_per_xmr: our_price.map(|d| d.to_string()),
        cex_btc_per_xmr: mid.map(|d| d.to_string()),
        rank_by_price: our_rank,
        total_active: total,
        cheapest_competitor_spread_pct: cheapest.map(|d| d.to_string()),
        trend_30m: trend_from_snapshots(),
    })
}

fn trend_from_snapshots() -> Vec<ChartPoint> {
    // v1: empty; the page just won't render a sparkline.
    Vec::new()
}
