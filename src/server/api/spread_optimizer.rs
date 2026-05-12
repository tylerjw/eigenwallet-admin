//! Auto-spread recommendation engine. Reads recent swap profitability,
//! competitor scan results, CEX volatility, and current inventory, and
//! emits a target spread for the maker. Adapted from Avellaneda–Stoikov
//! (2008) with this market's specific frictions (~30-60 min settlement
//! window, on-chain fees both legs, rebalancing cost) baked in.
//!
//! The formula:
//!   recommended = max(
//!       floor,
//!       vol_term + inventory_term + competitor_term + margin_term,
//!   )
//! clamped to [min_spread, max_spread]; step-capped per cycle.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde_json::json;

use crate::server::db;
use crate::server::schema::{
    balance_snapshots, cex_prices, spread_optimizer_config, spread_recommendations, swaps,
};
use crate::server::state::AppStateInner;
use crate::types::{
    SpreadOptimizerComponentsDto, SpreadOptimizerConfigDto, SpreadOptimizerRecommendationDto,
};

const SATS_PER_BTC: i64 = 100_000_000;

pub async fn get_config(state: &AppStateInner) -> Result<SpreadOptimizerConfigDto> {
    let mut conn = db::checkout(&state.pool).await?;
    type Row = (
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        i32,
        bool,
    );
    let row: Row = spread_optimizer_config::table
        .select((
            spread_optimizer_config::gamma,
            spread_optimizer_config::min_spread,
            spread_optimizer_config::max_spread,
            spread_optimizer_config::target_swap_profit_usd,
            spread_optimizer_config::amortized_recycle_cost_usd,
            spread_optimizer_config::chain_fees_per_swap_usd,
            spread_optimizer_config::step_size_max,
            spread_optimizer_config::cooldown_seconds,
            spread_optimizer_config::auto_apply,
        ))
        .first(&mut *conn)
        .await?;
    Ok(SpreadOptimizerConfigDto {
        gamma: row.0.to_string(),
        min_spread: row.1.to_string(),
        max_spread: row.2.to_string(),
        target_swap_profit_usd: row.3.to_string(),
        amortized_recycle_cost_usd: row.4.to_string(),
        chain_fees_per_swap_usd: row.5.to_string(),
        step_size_max: row.6.to_string(),
        cooldown_seconds: row.7,
        auto_apply: row.8,
    })
}

pub async fn save_config(state: &AppStateInner, cfg: SpreadOptimizerConfigDto) -> Result<()> {
    let parse = |s: &str, name: &str| -> Result<Decimal> {
        s.parse::<Decimal>()
            .map_err(|_| anyhow!("invalid decimal for {name}: {s}"))
    };
    let mut conn = db::checkout(&state.pool).await?;
    diesel::update(spread_optimizer_config::table.filter(spread_optimizer_config::id.eq(1)))
        .set((
            spread_optimizer_config::gamma.eq(parse(&cfg.gamma, "gamma")?),
            spread_optimizer_config::min_spread.eq(parse(&cfg.min_spread, "min_spread")?),
            spread_optimizer_config::max_spread.eq(parse(&cfg.max_spread, "max_spread")?),
            spread_optimizer_config::target_swap_profit_usd.eq(parse(
                &cfg.target_swap_profit_usd,
                "target_swap_profit_usd",
            )?),
            spread_optimizer_config::amortized_recycle_cost_usd.eq(parse(
                &cfg.amortized_recycle_cost_usd,
                "amortized_recycle_cost_usd",
            )?),
            spread_optimizer_config::chain_fees_per_swap_usd.eq(parse(
                &cfg.chain_fees_per_swap_usd,
                "chain_fees_per_swap_usd",
            )?),
            spread_optimizer_config::step_size_max.eq(parse(&cfg.step_size_max, "step_size_max")?),
            spread_optimizer_config::cooldown_seconds.eq(cfg.cooldown_seconds),
            spread_optimizer_config::auto_apply.eq(cfg.auto_apply),
            spread_optimizer_config::updated_at.eq(chrono::Utc::now()),
        ))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Compute and persist a recommendation. If `apply` and the optimizer is in
/// step-size + cooldown bounds, also pushes the new spread to the asb
/// ConfigMap via `maker::write_config`.
pub async fn recommend(state: &AppStateInner) -> Result<SpreadOptimizerRecommendationDto> {
    let cfg = get_config(state).await?;
    let gamma: Decimal = cfg.gamma.parse()?;
    let min_spread: Decimal = cfg.min_spread.parse()?;
    let max_spread: Decimal = cfg.max_spread.parse()?;
    let target_profit: Decimal = cfg.target_swap_profit_usd.parse()?;
    let recycle_cost: Decimal = cfg.amortized_recycle_cost_usd.parse()?;
    let chain_fees: Decimal = cfg.chain_fees_per_swap_usd.parse()?;

    // ---- Inputs --------------------------------------------------------
    let now = chrono::Utc::now();
    let mut conn = db::checkout(&state.pool).await?;

    // Average swap size (USD), last 30 days. Default to $200 if no data.
    type SwapRow = (i64, Option<Decimal>, DateTime<Utc>);
    let recent_swaps: Vec<SwapRow> = swaps::table
        .filter(swaps::completed_at.is_not_null())
        .filter(swaps::completed_at.ge(now - ChronoDuration::days(30)))
        .filter(swaps::btc_usd_at_completion.is_not_null())
        .select((
            swaps::btc_sat,
            swaps::btc_usd_at_completion,
            swaps::started_at,
        ))
        .load(&mut *conn)
        .await
        .unwrap_or_default();

    let avg_swap_usd: Decimal = if recent_swaps.is_empty() {
        Decimal::from(200)
    } else {
        let sum: Decimal = recent_swaps
            .iter()
            .filter_map(|(sat, btc_usd, _)| {
                btc_usd.map(|p| Decimal::from(*sat) / Decimal::from(SATS_PER_BTC) * p)
            })
            .sum();
        sum / Decimal::from(recent_swaps.len() as i64)
    };

    // BTC/USD 30-min return volatility from cex_prices, last 7 days.
    let cex_rows: Vec<(DateTime<Utc>, Option<Decimal>)> = cex_prices::table
        .filter(cex_prices::sampled_at.ge(now - ChronoDuration::days(7)))
        .filter(cex_prices::btc_usd.is_not_null())
        .select((cex_prices::sampled_at, cex_prices::btc_usd))
        .order(cex_prices::sampled_at.asc())
        .load(&mut *conn)
        .await
        .unwrap_or_default();
    let vol_30min = compute_30min_return_vol(&cex_rows);

    // Inventory skew. +1 = all BTC, -1 = all XMR, 0 = balanced (USD-weighted).
    let snap: Option<(i64, Decimal, Decimal, Decimal)> = balance_snapshots::table
        .filter(balance_snapshots::btc_usd.gt(Decimal::ZERO))
        .filter(balance_snapshots::xmr_usd.gt(Decimal::ZERO))
        .order(balance_snapshots::taken_at.desc())
        .select((
            balance_snapshots::btc_sat,
            balance_snapshots::xmr_atomic,
            balance_snapshots::btc_usd,
            balance_snapshots::xmr_usd,
        ))
        .first(&mut *conn)
        .await
        .optional()?;
    let inventory_skew = snap
        .map(|(sat, xmr_atomic, btc_p, xmr_p)| {
            let btc_v = Decimal::from(sat) / Decimal::from(SATS_PER_BTC) * btc_p;
            let xmr_v = xmr_atomic / Decimal::from(1_000_000_000_000i64) * xmr_p;
            let total = btc_v + xmr_v;
            if total.is_zero() {
                Decimal::ZERO
            } else {
                ((btc_v - xmr_v) / total).round_dp(4)
            }
        })
        .unwrap_or(Decimal::ZERO);

    drop(conn);

    // Competitor tier-1 cutoff from latest scan (reuse existing logic).
    let scan = crate::server::api::competitors::latest(state)
        .await
        .ok()
        .flatten();
    let mut comp_spreads: Vec<Decimal> = scan
        .as_ref()
        .map(|s| {
            s.quotes
                .iter()
                .filter(|q| q.reachable && !q.is_us)
                .filter_map(|q| q.spread_vs_cex_pct.as_ref().and_then(|s| s.parse().ok()))
                .collect()
        })
        .unwrap_or_default();
    comp_spreads.sort();
    // Tier-1 cutoff = 3rd-cheapest competitor spread (expressed as a %, so divide by 100 to get fraction).
    let tier1_pct = comp_spreads.get(2).copied();
    let tier1_frac = tier1_pct.map(|p| p / Decimal::from(100));

    let current_quote = state.asb.get_current_quote().await.ok();
    let our_price = current_quote
        .as_ref()
        .map(|q| Decimal::from(q.price) / Decimal::from(SATS_PER_BTC));
    let cex_snap = state.cex.read().await.last.clone();
    let mid_btc_per_xmr = cex_snap.as_ref().and_then(|s| s.btc_xmr);
    let current_spread = match (our_price, mid_btc_per_xmr) {
        (Some(p), Some(m)) if !m.is_zero() => (p - m) / m,
        _ => Decimal::ZERO,
    };

    // ---- Components ----------------------------------------------------
    // Floor: minimum spread that covers chain fees + amortized recycle cost
    // for a typical swap size. (fees + recycle) / avg_swap_size_usd.
    let floor_cost = chain_fees + recycle_cost + target_profit;
    let floor = if avg_swap_usd.is_zero() {
        min_spread
    } else {
        (floor_cost / avg_swap_usd).round_dp(4)
    };

    // Vol term: γ × σ²(T-t) approximated. We use σ over a single 30-min
    // window (the settlement horizon) and scale linearly by γ.
    let vol_term = (gamma * vol_30min).round_dp(4);

    // Inventory term: γ × |skew| × σ. When we're heavily on one side,
    // widen so customers prefer the *other* direction (reducing imbalance).
    let inventory_term = (gamma * inventory_skew.abs() * vol_30min * Decimal::from(2)).round_dp(4);

    // Competitor term: if we're not already inside tier-1, don't penalize
    // ourselves further — let the floor/vol/inv set the price. If we are
    // inside tier-1 *and* the tier-1 cutoff is well above our floor, push
    // up toward the cutoff. We expose this as max(0, tier1_frac - vol_term).
    let competitor_term = match tier1_frac {
        Some(t) if t > Decimal::ZERO => {
            let headroom = t - vol_term - inventory_term;
            if headroom > Decimal::ZERO {
                (headroom / Decimal::from(2)).round_dp(4)
            } else {
                Decimal::ZERO
            }
        }
        _ => Decimal::ZERO,
    };

    let margin_term = if avg_swap_usd.is_zero() {
        Decimal::ZERO
    } else {
        (target_profit / avg_swap_usd).round_dp(4)
    };

    let sum_terms = vol_term + inventory_term + competitor_term + margin_term;
    let raw = floor.max(sum_terms);
    let clamped = raw.max(min_spread).min(max_spread);

    // ---- Step cap vs the current applied spread -----------------------
    let step: Decimal = cfg.step_size_max.parse()?;
    let recommended = if (clamped - current_spread).abs() <= step {
        clamped
    } else if clamped > current_spread {
        (current_spread + step).round_dp(4)
    } else {
        (current_spread - step).round_dp(4)
    };

    let rationale = build_rationale(
        current_spread,
        recommended,
        floor,
        vol_term,
        inventory_term,
        competitor_term,
        margin_term,
        vol_30min,
        inventory_skew,
        tier1_pct,
    );

    let components = SpreadOptimizerComponentsDto {
        floor: floor.to_string(),
        vol_term: vol_term.to_string(),
        inventory_term: inventory_term.to_string(),
        competitor_term: competitor_term.to_string(),
        margin_term: margin_term.to_string(),
        raw_vol_30min: vol_30min.to_string(),
        inventory_skew: inventory_skew.to_string(),
        tier1_cutoff_pct: tier1_pct.map(|d| d.to_string()),
        avg_swap_usd: avg_swap_usd.round_dp(2).to_string(),
        clamped_to_bounds: clamped != raw,
        step_capped: clamped != recommended,
    };

    Ok(SpreadOptimizerRecommendationDto {
        recommended_at: chrono::Utc::now(),
        current_spread: current_spread.round_dp(4).to_string(),
        recommended_spread: recommended.to_string(),
        components,
        rationale,
        auto_apply: cfg.auto_apply,
    })
}

/// Persist the most recent recommendation into spread_recommendations.
/// Returns the inserted row id.
pub async fn save_recommendation(
    state: &AppStateInner,
    rec: &SpreadOptimizerRecommendationDto,
) -> Result<i64> {
    let mut conn = db::checkout(&state.pool).await?;
    let components_json = json!({
        "floor": rec.components.floor,
        "vol_term": rec.components.vol_term,
        "inventory_term": rec.components.inventory_term,
        "competitor_term": rec.components.competitor_term,
        "margin_term": rec.components.margin_term,
        "raw_vol_30min": rec.components.raw_vol_30min,
        "inventory_skew": rec.components.inventory_skew,
        "tier1_cutoff_pct": rec.components.tier1_cutoff_pct,
        "avg_swap_usd": rec.components.avg_swap_usd,
        "clamped_to_bounds": rec.components.clamped_to_bounds,
        "step_capped": rec.components.step_capped,
    });
    let id: i64 = diesel::insert_into(spread_recommendations::table)
        .values((
            spread_recommendations::current_spread.eq(rec.current_spread.parse::<Decimal>()?),
            spread_recommendations::recommended_spread
                .eq(rec.recommended_spread.parse::<Decimal>()?),
            spread_recommendations::components.eq(components_json),
            spread_recommendations::rationale.eq(&rec.rationale),
        ))
        .returning(spread_recommendations::id)
        .get_result(&mut *conn)
        .await?;
    Ok(id)
}

/// Apply a recommendation to the asb ConfigMap and mark the row as applied.
/// Caller is responsible for cooldown / bounds checks.
pub async fn apply_recommendation(state: &AppStateInner, rec_id: i64) -> Result<()> {
    let mut conn = db::checkout(&state.pool).await?;
    let recommended_spread: Decimal = spread_recommendations::table
        .filter(spread_recommendations::id.eq(rec_id))
        .select(spread_recommendations::recommended_spread)
        .first(&mut *conn)
        .await?;

    let current_cfg = crate::server::api::maker::read_config(state).await?;
    let update = crate::types::MakerConfigUpdate {
        min_buy_btc: current_cfg.min_buy_btc,
        max_buy_btc: current_cfg.max_buy_btc,
        ask_spread: recommended_spread.to_string(),
        developer_tip: current_cfg.developer_tip,
        anti_spam_deposit_ratio: current_cfg.anti_spam_deposit_ratio,
    };
    crate::server::api::maker::write_config(state, update).await?;

    diesel::update(spread_recommendations::table.filter(spread_recommendations::id.eq(rec_id)))
        .set((
            spread_recommendations::applied.eq(true),
            spread_recommendations::applied_at.eq(chrono::Utc::now()),
        ))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Most recent spread_recommendations row that was applied (used by the
/// poller to enforce cooldowns).
pub async fn last_applied_at(state: &AppStateInner) -> Result<Option<DateTime<Utc>>> {
    let mut conn = db::checkout(&state.pool).await?;
    let at: Option<DateTime<Utc>> = spread_recommendations::table
        .filter(spread_recommendations::applied.eq(true))
        .filter(spread_recommendations::applied_at.is_not_null())
        .order(spread_recommendations::applied_at.desc())
        .select(spread_recommendations::applied_at)
        .first::<Option<DateTime<Utc>>>(&mut *conn)
        .await
        .optional()?
        .flatten();
    Ok(at)
}

fn compute_30min_return_vol(rows: &[(DateTime<Utc>, Option<Decimal>)]) -> Decimal {
    // Build a sorted-by-time sequence of prices; sample every ~30 min
    // window for log-returns; report population stddev.
    let mut prices: Vec<(DateTime<Utc>, f64)> = rows
        .iter()
        .filter_map(|(t, p)| {
            p.map(|d| (*t, d.to_f64().unwrap_or(0.0)))
                .filter(|(_, p)| *p > 0.0)
        })
        .collect();
    if prices.len() < 4 {
        return Decimal::from_f64(0.005).unwrap_or(Decimal::ZERO); // fallback 0.5%
    }
    prices.sort_by_key(|(t, _)| *t);

    let window = ChronoDuration::minutes(30);
    let mut sampled: Vec<f64> = vec![prices[0].1];
    let mut last_t = prices[0].0;
    for (t, p) in prices.iter().skip(1) {
        if *t - last_t >= window {
            sampled.push(*p);
            last_t = *t;
        }
    }
    if sampled.len() < 3 {
        return Decimal::from_f64(0.005).unwrap_or(Decimal::ZERO);
    }

    let mut rets: Vec<f64> = Vec::with_capacity(sampled.len() - 1);
    for w in sampled.windows(2) {
        if w[0] > 0.0 {
            rets.push((w[1] / w[0]).ln());
        }
    }
    if rets.is_empty() {
        return Decimal::from_f64(0.005).unwrap_or(Decimal::ZERO);
    }
    let mean = rets.iter().sum::<f64>() / rets.len() as f64;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rets.len() as f64;
    let sd = var.sqrt();
    Decimal::from_f64(sd).unwrap_or(Decimal::ZERO).round_dp(5)
}

#[allow(clippy::too_many_arguments)]
fn build_rationale(
    current: Decimal,
    recommended: Decimal,
    floor: Decimal,
    vol_term: Decimal,
    inv_term: Decimal,
    comp_term: Decimal,
    margin_term: Decimal,
    vol_30min: Decimal,
    inv_skew: Decimal,
    tier1: Option<Decimal>,
) -> String {
    let direction = if recommended > current {
        "widen"
    } else if recommended < current {
        "tighten"
    } else {
        "hold"
    };
    let to_pct = |d: Decimal| (d * Decimal::from(100)).round_dp(3);
    let tier1_str = tier1
        .map(|t| format!(", tier-1 cutoff {t}%"))
        .unwrap_or_default();
    format!(
        "{} → {} (current {}). floor {}%, vol {}% (σ_30min={}%), inv {}% (skew {}), comp {}%, margin {}%{}.",
        direction,
        to_pct(recommended),
        to_pct(current),
        to_pct(floor),
        to_pct(vol_term),
        to_pct(vol_30min),
        to_pct(inv_term),
        inv_skew,
        to_pct(comp_term),
        to_pct(margin_term),
        tier1_str,
    )
}
