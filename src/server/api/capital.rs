use anyhow::{Result, anyhow};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;

use crate::server::db;
use crate::server::models::{CapitalEvent, NewCapitalEvent};
use crate::server::schema::capital_events;
use crate::server::state::AppStateInner;
use crate::types::{CapitalEventDto, CapitalEventInput};

pub async fn list(state: &AppStateInner) -> Result<Vec<CapitalEventDto>> {
    let mut conn = db::checkout(&state.pool).await?;
    let rows: Vec<CapitalEvent> = capital_events::table
        .select(CapitalEvent::as_select())
        .order(capital_events::occurred_at.desc())
        .load(&mut *conn)
        .await?;
    Ok(rows
        .into_iter()
        .map(|e| CapitalEventDto {
            id: e.id.to_string(),
            occurred_at: e.occurred_at,
            direction: e.direction,
            asset: e.asset,
            amount_atomic: e.amount_atomic.to_string(),
            usd_value_at_event: e.usd_value_at_event.map(|d| d.to_string()),
            notes: e.notes,
        })
        .collect())
}

pub async fn add(state: &AppStateInner, input: CapitalEventInput) -> Result<CapitalEventDto> {
    if input.direction != "deposit" && input.direction != "withdraw" {
        return Err(anyhow!("direction must be deposit|withdraw"));
    }
    if input.asset != "BTC" && input.asset != "XMR" {
        return Err(anyhow!("asset must be BTC|XMR"));
    }
    // Convert human-readable amount → atomic units. BTC: ×1e8, XMR: ×1e12.
    let human: Decimal = input
        .amount
        .trim()
        .parse()
        .map_err(|_| anyhow!("amount '{}' not numeric", input.amount))?;
    let multiplier = match input.asset.as_str() {
        "BTC" => Decimal::from(100_000_000i64),
        "XMR" => Decimal::from(1_000_000_000_000i64),
        _ => unreachable!(),
    };
    let amount = (human * multiplier).round();

    // USD: explicit override, else live CEX price (only meaningful for recent
    // events; historical events should pass an explicit override).
    let usd: Option<Decimal> = match input.usd_value_at_event {
        Some(s) if !s.trim().is_empty() => Some(
            s.parse()
                .map_err(|_| anyhow!("usd_value_at_event not numeric"))?,
        ),
        _ => {
            let snap = state.cex.read().await.last.clone();
            let now = chrono::Utc::now();
            // Only auto-fill if the event is within an hour of now; otherwise
            // the live price would be misleading for a historical entry.
            let recent = (now - input.occurred_at).num_minutes().abs() < 60;
            if recent {
                match (snap, input.asset.as_str()) {
                    (Some(s), "BTC") => s.btc_usd.map(|p| (human * p).round_dp(2)),
                    (Some(s), "XMR") => s.xmr_usd.map(|p| (human * p).round_dp(2)),
                    _ => None,
                }
            } else {
                None
            }
        }
    };
    let mut conn = db::checkout(&state.pool).await?;
    let notes = input.notes.and_then(|n| {
        let t = n.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });
    let row: CapitalEvent = diesel::insert_into(capital_events::table)
        .values(NewCapitalEvent {
            occurred_at: input.occurred_at,
            direction: input.direction,
            asset: input.asset,
            amount_atomic: amount,
            usd_value_at_event: usd,
            notes,
        })
        .returning(CapitalEvent::as_select())
        .get_result(&mut *conn)
        .await?;
    Ok(CapitalEventDto {
        id: row.id.to_string(),
        occurred_at: row.occurred_at,
        direction: row.direction,
        asset: row.asset,
        amount_atomic: row.amount_atomic.to_string(),
        usd_value_at_event: row.usd_value_at_event.map(|d| d.to_string()),
        notes: row.notes,
    })
}
