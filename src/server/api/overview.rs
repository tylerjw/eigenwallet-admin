use anyhow::Result;
use chrono::Duration as ChronoDuration;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::schema::swaps;
use crate::server::state::AppStateInner;
use crate::types::{OverviewDto, QuoteDto, RegistrationDto, RendezvousRegistration};

const SATS_PER_BTC: i64 = 100_000_000;

pub async fn fetch(state: &AppStateInner) -> Result<OverviewDto> {
    let btc = state.asb.bitcoin_balance().await.ok();
    let xmr = state.asb.monero_balance().await.ok();
    let peers = state.asb.active_connections().await.ok();
    let reg = state.asb.registration_status().await.ok();
    let multi = state.asb.multiaddresses().await.unwrap_or_default();
    let quote = state.asb.get_current_quote().await.ok();
    let active_swaps = state
        .asb
        .get_swaps()
        .await
        .map(|v| v.iter().filter(|s| !s.completed).count() as i32)
        .unwrap_or(0);

    let swaps_24h = {
        let cutoff = chrono::Utc::now() - ChronoDuration::hours(24);
        let mut conn = db::checkout(&state.pool).await?;
        swaps::table
            .filter(swaps::completed_at.ge(cutoff))
            .count()
            .get_result::<i64>(&mut *conn)
            .await
            .unwrap_or(0) as i32
    };

    let snap = state.cex.read().await.last.clone();
    let btc_usd = snap.as_ref().and_then(|s| s.btc_usd);
    let xmr_usd = snap.as_ref().and_then(|s| s.xmr_usd);

    let btc_sat = btc.as_ref().map(|b| b.balance).unwrap_or(0);
    let xmr_atomic_u: u128 = xmr.as_ref().map(|x| x.balance).unwrap_or(0);
    let xmr_atomic_str = xmr_atomic_u.to_string();
    let xmr_atomic_dec: Decimal = xmr_atomic_str.parse().unwrap_or(Decimal::ZERO);
    let xmr_dec = xmr_atomic_dec / Decimal::from(1_000_000_000_000i64);
    let btc_dec = Decimal::from(btc_sat) / Decimal::from(SATS_PER_BTC);
    let total_usd = match (btc_usd, xmr_usd) {
        (Some(b), Some(x)) => Some((btc_dec * b + xmr_dec * x).to_string()),
        _ => None,
    };

    let registration = reg.map(|r| RegistrationDto {
        registered: r.registered_count(),
        total: r.total(),
        details: r
            .registrations
            .into_iter()
            .map(|e| RendezvousRegistration {
                multiaddr: e.address,
                status: format!("{}/{}", e.connection, e.registration),
            })
            .collect(),
    });

    // Quote: price/min/max are integer satoshi. Convert to BTC decimal for UI.
    let cex_btc_per_xmr = snap.as_ref().and_then(|s| s.btc_xmr);
    let (current_quote, spread_pct) = match quote {
        Some(q) => {
            let price_btc = Decimal::from(q.price) / Decimal::from(SATS_PER_BTC);
            let min_btc = Decimal::from(q.min_quantity) / Decimal::from(SATS_PER_BTC);
            let max_btc = Decimal::from(q.max_quantity) / Decimal::from(SATS_PER_BTC);
            let spread = cex_btc_per_xmr
                .filter(|m| !m.is_zero())
                .map(|mid| ((price_btc - mid) / mid * Decimal::from(100)).round_dp(2));
            (
                Some(QuoteDto {
                    price_btc_per_xmr: price_btc.to_string(),
                    min_btc: min_btc.to_string(),
                    max_btc: max_btc.to_string(),
                    spread_pct: spread.map(|d| d.to_string()),
                }),
                spread,
            )
        }
        None => (None, None),
    };
    let _ = spread_pct;

    Ok(OverviewDto {
        btc_balance_sat: btc_sat,
        xmr_balance_atomic: xmr_atomic_str,
        btc_usd: btc_usd.map(|d| d.to_string()),
        xmr_usd: xmr_usd.map(|d| d.to_string()),
        total_usd,
        peer_count: peers,
        registration,
        active_swaps,
        swaps_24h,
        onion_addresses: multi.into_iter().filter(|a| a.contains(".onion")).collect(),
        current_quote,
        as_of: chrono::Utc::now(),
    })
}
