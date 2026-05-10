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
    let amount: Decimal = input
        .amount_atomic
        .parse()
        .map_err(|_| anyhow!("amount_atomic not numeric"))?;
    let usd: Option<Decimal> = match input.usd_value_at_event {
        Some(s) if !s.trim().is_empty() => Some(
            s.parse()
                .map_err(|_| anyhow!("usd_value_at_event not numeric"))?,
        ),
        _ => None,
    };
    let mut conn = db::checkout(&state.pool).await?;
    let row: CapitalEvent = diesel::insert_into(capital_events::table)
        .values(NewCapitalEvent {
            occurred_at: input.occurred_at,
            direction: input.direction,
            asset: input.asset,
            amount_atomic: amount,
            usd_value_at_event: usd,
            notes: input.notes,
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
