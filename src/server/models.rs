//! diesel ORM types. These are inserted/queried; DTOs in `crate::types` are the
//! over-the-wire shape and are deliberately string-based for cross-wasm portability.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::schema::*;

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = admin_credentials)]
pub struct AdminCredential {
    pub id: Uuid,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub rotated_at: Option<DateTime<Utc>>,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = balance_snapshots)]
pub struct BalanceSnapshot {
    pub taken_at: DateTime<Utc>,
    pub btc_sat: i64,
    pub xmr_atomic: Decimal,
    pub btc_usd: Decimal,
    pub xmr_usd: Decimal,
    pub total_usd: Decimal,
    pub total_btc: Decimal,
}

#[derive(Queryable, Selectable, Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = swaps)]
pub struct Swap {
    pub swap_id: String,
    pub peer_id: String,
    pub state: String,
    pub btc_sat: i64,
    pub xmr_atomic: Decimal,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub btc_usd_at_completion: Option<Decimal>,
    pub xmr_usd_at_completion: Option<Decimal>,
    pub profit_usd: Option<Decimal>,
    pub raw_log_excerpt: Option<serde_json::Value>,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = capital_events)]
pub struct CapitalEvent {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub direction: String,
    pub asset: String,
    pub amount_atomic: Decimal,
    pub usd_value_at_event: Option<Decimal>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = capital_events)]
pub struct NewCapitalEvent {
    pub occurred_at: DateTime<Utc>,
    pub direction: String,
    pub asset: String,
    pub amount_atomic: Decimal,
    pub usd_value_at_event: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = cex_prices)]
pub struct CexPrice {
    pub sampled_at: DateTime<Utc>,
    pub btc_usd: Option<Decimal>,
    pub xmr_usd: Option<Decimal>,
    pub btc_xmr: Option<Decimal>,
    pub sources: Vec<String>,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = competitor_scans)]
pub struct CompetitorScan {
    pub scan_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub trigger: String,
    pub raw_output: Option<String>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = competitor_scans)]
pub struct NewCompetitorScan {
    pub scan_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub trigger: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = competitor_quotes)]
pub struct CompetitorQuote {
    pub id: i64,
    pub scan_id: Uuid,
    pub peer_id: String,
    pub multiaddr: Option<String>,
    pub price_btc_per_xmr: Option<Decimal>,
    pub min_btc: Option<Decimal>,
    pub max_btc: Option<Decimal>,
    pub reachable: bool,
    pub reason_if_unreachable: Option<String>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = competitor_quotes)]
pub struct NewCompetitorQuote {
    pub scan_id: Uuid,
    pub peer_id: String,
    pub multiaddr: Option<String>,
    pub price_btc_per_xmr: Option<Decimal>,
    pub min_btc: Option<Decimal>,
    pub max_btc: Option<Decimal>,
    pub reachable: bool,
    pub reason_if_unreachable: Option<String>,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = maker_config_history)]
pub struct MakerConfigHistory {
    pub id: i64,
    pub changed_at: DateTime<Utc>,
    pub previous_toml: String,
    pub new_toml: String,
    pub restart_observed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = maker_config_history)]
pub struct NewMakerConfigHistory {
    pub previous_toml: String,
    pub new_toml: String,
    pub notes: Option<String>,
}
