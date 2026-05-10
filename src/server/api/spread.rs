use anyhow::Result;
use rust_decimal::Decimal;

use crate::server::state::AppStateInner;
use crate::types::SpreadRecommendationDto;

const SATS_PER_BTC: i64 = 100_000_000;
const TIER_N: usize = 3;
const STAY_BAND_PCT: Decimal = rust_decimal_macros::dec!(0.5);
const SAFETY_MARGIN_PCT: Decimal = rust_decimal_macros::dec!(0.1);

pub async fn recommend(state: &AppStateInner) -> Result<SpreadRecommendationDto> {
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

    let scan = super::competitors::latest(state).await.ok().flatten();
    let mut comps: Vec<Decimal> = scan
        .as_ref()
        .map(|s| {
            s.quotes
                .iter()
                .filter(|q| q.reachable)
                .filter_map(|q| {
                    q.spread_vs_cex_pct
                        .as_ref()
                        .and_then(|s| s.parse::<Decimal>().ok())
                })
                .collect()
        })
        .unwrap_or_default();
    comps.sort();

    let tier1 = comps.get(TIER_N.saturating_sub(1)).copied();

    let (recommended, reasoning) = match (our_spread, tier1) {
        (Some(us), Some(t1)) if us > t1 => {
            let r = (t1 - SAFETY_MARGIN_PCT).round_dp(2);
            (
                Some(r),
                format!(
                    "You're at +{us}%, tier-1 cutoff is +{t1}%. Tighten to +{r}% to enter the top {TIER_N}."
                ),
            )
        }
        (Some(us), Some(t1)) if us <= t1 && us >= t1 - STAY_BAND_PCT => (
            Some(us),
            format!("You're at +{us}%, comfortably inside tier-1 (cutoff +{t1}%). Stay put."),
        ),
        (Some(us), Some(t1)) if us < t1 - STAY_BAND_PCT => {
            let r = (t1 - SAFETY_MARGIN_PCT).round_dp(2);
            (
                Some(r),
                format!(
                    "You're at +{us}%, but tier-1 cutoff is +{t1}%. You could widen to +{r}% without losing position."
                ),
            )
        }
        (Some(us), None) => (
            Some(us),
            format!("You're at +{us}%; no fresh competitor scan to compare against."),
        ),
        (None, _) => (None, "Current quote unavailable.".into()),
        _ => (None, "Insufficient data.".into()),
    };

    let our_rank = our_spread.map(|us| (comps.iter().filter(|c| **c < us).count() as i32) + 1);

    Ok(SpreadRecommendationDto {
        current_spread_pct: our_spread.map(|d| d.to_string()),
        recommended_spread_pct: recommended.map(|d| d.to_string()),
        reasoning,
        tier_1_cutoff_pct: tier1.map(|d| d.to_string()),
        our_rank,
    })
}
