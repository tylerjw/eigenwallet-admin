//! Background poller that periodically computes a spread recommendation
//! and (if `auto_apply` is on in the optimizer config) pushes it to asb
//! after cooldown + step-size guardrails. Always persists the
//! recommendation in `spread_recommendations` for audit, regardless of
//! whether it was applied.

use chrono::Utc;
use std::time::Duration;
use tokio::time::interval;

use crate::server::state::AppState;

const POLL_EVERY: Duration = Duration::from_secs(900); // 15 min

pub async fn run(state: AppState) {
    // Stagger first run by a few seconds so we don't fight the other
    // startup pollers for the DB pool.
    tokio::time::sleep(Duration::from_secs(30)).await;
    let mut tick = interval(POLL_EVERY);
    loop {
        tick.tick().await;
        if let Err(e) = run_once(&state).await {
            tracing::warn!(error = %e, "spread-optimizer tick failed");
        }
    }
}

async fn run_once(state: &AppState) -> anyhow::Result<()> {
    // Bail if maker is paused — don't fight the operator's intentional pause.
    if crate::server::api::maker::get_pause_state(&state.0)
        .await
        .map(|p| p.is_paused)
        .unwrap_or(false)
    {
        tracing::debug!("spread-optimizer: maker is paused, skipping");
        return Ok(());
    }

    let rec = crate::server::api::spread_optimizer::recommend(&state.0).await?;
    let cfg = crate::server::api::spread_optimizer::get_config(&state.0).await?;
    let id = crate::server::api::spread_optimizer::save_recommendation(&state.0, &rec).await?;

    if !cfg.auto_apply {
        tracing::debug!(
            id,
            current = %rec.current_spread,
            recommended = %rec.recommended_spread,
            "spread-optimizer: recorded recommendation (auto-apply off)",
        );
        return Ok(());
    }

    // Skip apply if the recommendation hasn't moved meaningfully.
    let current: rust_decimal::Decimal = rec.current_spread.parse()?;
    let recommended: rust_decimal::Decimal = rec.recommended_spread.parse()?;
    let step: rust_decimal::Decimal = cfg.step_size_max.parse()?;
    // Don't apply micro-changes (≤ 10% of the step cap).
    let min_meaningful = step / rust_decimal::Decimal::from(10);
    if (recommended - current).abs() < min_meaningful {
        tracing::debug!("spread-optimizer: change below noise floor, skipping apply");
        return Ok(());
    }

    // Cooldown: no consecutive applies within `cooldown_seconds`.
    if let Some(last) = crate::server::api::spread_optimizer::last_applied_at(&state.0).await? {
        let elapsed = (Utc::now() - last).num_seconds();
        if elapsed < i64::from(cfg.cooldown_seconds) {
            tracing::debug!(
                elapsed,
                cooldown = cfg.cooldown_seconds,
                "spread-optimizer: cooldown not elapsed, skipping apply",
            );
            return Ok(());
        }
    }

    tracing::info!(
        id,
        current = %rec.current_spread,
        recommended = %rec.recommended_spread,
        "spread-optimizer: auto-applying",
    );
    crate::server::api::spread_optimizer::apply_recommendation(&state.0, id).await?;
    Ok(())
}
