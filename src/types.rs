//! DTOs shared between the server and the Leptos client.
//! Plain serde types; no diesel/sqlx imports here so they compile to wasm.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverviewDto {
    pub btc_balance_sat: i64,
    pub xmr_balance_atomic: String,
    pub btc_usd: Option<String>,
    pub xmr_usd: Option<String>,
    pub total_usd: Option<String>,
    pub peer_count: Option<i32>,
    pub registration: Option<RegistrationDto>,
    pub active_swaps: i32,
    pub onion_addresses: Vec<String>,
    pub current_quote: Option<QuoteDto>,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistrationDto {
    pub registered: i32,
    pub total: i32,
    pub details: Vec<RendezvousRegistration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RendezvousRegistration {
    pub multiaddr: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuoteDto {
    pub price_btc_per_xmr: String,
    pub min_btc: String,
    pub max_btc: String,
    pub spread_pct: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthDto {
    pub asb: SubsystemHealth,
    pub bitcoind: SubsystemHealth,
    pub monerod: SubsystemHealth,
    pub electrs: SubsystemHealth,
    pub tor: SubsystemHealth,
    pub peers: SubsystemHealth,
    pub rendezvous: SubsystemHealth,
    pub admin_db: SubsystemHealth,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubsystemHealth {
    pub state: HealthState,
    pub headline: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Ok,
    Degraded,
    Down,
    Unknown,
}

impl HealthState {
    pub fn badge_class(self) -> &'static str {
        match self {
            HealthState::Ok => "badge-ok",
            HealthState::Degraded => "badge-warn",
            HealthState::Down | HealthState::Unknown => "badge-err",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapRow {
    pub swap_id: String,
    pub peer_id: String,
    pub state: String,
    pub btc_sat: i64,
    pub xmr_atomic: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub profit_usd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapListDto {
    pub total: i64,
    pub rows: Vec<SwapRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChartPoint {
    pub t: DateTime<Utc>,
    pub v: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChartSeries {
    pub points: Vec<ChartPoint>,
    pub denomination: String,
    pub period: String,
}

/// P&L attribution: decomposes the change in total portfolio value over a
/// period into three components. Identity:
///   end_value - start_value = market_pnl + trade_pnl + capital_flow
///
/// - `market_pnl`: value change from price moves on existing holdings
///   (between snapshots, holdings(t-1) × (price(t) - price(t-1)))
/// - `trade_pnl`: value change from quantity changes priced at the post-trade
///   price, minus any external capital flow in the same interval. Captures
///   the spread the maker has captured (or surrendered) via swaps.
/// - `capital_flow`: net external deposits minus withdrawals (USD at event).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributionDto {
    /// Actual portfolio value over time, USD.
    pub actual: Vec<ChartPoint>,
    /// Hypothetical value if no swaps had occurred (price moves and external
    /// capital flow only).
    pub no_trade_baseline: Vec<ChartPoint>,
    pub start_value_usd: String,
    pub end_value_usd: String,
    pub market_pnl_usd: String,
    pub trade_pnl_usd: String,
    pub capital_flow_usd: String,
    pub period: String,
    pub sample_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MakerConfigDto {
    pub min_buy_btc: String,
    pub max_buy_btc: String,
    pub ask_spread: String,
    pub developer_tip: String,
    pub anti_spam_deposit_ratio: String,
    pub raw_toml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakerConfigUpdate {
    pub min_buy_btc: String,
    pub max_buy_btc: String,
    pub ask_spread: String,
    pub developer_tip: String,
    pub anti_spam_deposit_ratio: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MakerConfigUpdateResult {
    pub config_version: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompetitorScanDto {
    pub scan_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub trigger: String,
    pub quotes: Vec<CompetitorQuoteDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompetitorQuoteDto {
    pub peer_id: String,
    pub multiaddr: Option<String>,
    pub price_btc_per_xmr: Option<String>,
    pub min_btc: Option<String>,
    pub max_btc: Option<String>,
    pub reachable: bool,
    pub reason_if_unreachable: Option<String>,
    pub spread_vs_cex_pct: Option<String>,
    /// True for the row representing us in the rendered list.
    #[serde(default)]
    pub is_us: bool,
    /// asb / swap-cli version the competitor is running, e.g. "4.5.0".
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketPositionDto {
    pub our_spread_pct: Option<String>,
    pub our_price_btc_per_xmr: Option<String>,
    pub cex_btc_per_xmr: Option<String>,
    pub rank_by_price: Option<i32>,
    pub total_active: i32,
    pub cheapest_competitor_spread_pct: Option<String>,
    pub trend_30m: Vec<ChartPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpreadRecommendationDto {
    pub current_spread_pct: Option<String>,
    pub recommended_spread_pct: Option<String>,
    pub reasoning: String,
    pub tier_1_cutoff_pct: Option<String>,
    pub our_rank: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoiDto {
    pub method: String,
    pub denomination: String,
    pub since: DateTime<Utc>,
    pub start_value: String,
    pub current_value: String,
    pub pct_change: String,
    pub days_elapsed: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalEventInput {
    /// When this capital event happened. Send as RFC3339 (`2026-04-15T14:30:00Z`)
    /// or chrono-default. UI sends datetime-local values converted to UTC.
    pub occurred_at: DateTime<Utc>,
    /// "deposit" or "withdraw"
    pub direction: String,
    /// "BTC" or "XMR"
    pub asset: String,
    /// Human-readable amount (e.g. "1.5" for 1.5 BTC). Server converts to
    /// atomic units (sat / piconero) based on `asset`.
    pub amount: String,
    /// USD value of the amount at the time of the event. Optional. If blank
    /// and the event is "recent" (within the CEX cache freshness), the server
    /// fills it from the live CEX price; otherwise it's stored as NULL and
    /// the operator can fill in historical price later.
    pub usd_value_at_event: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapitalEventDto {
    pub id: String,
    pub occurred_at: DateTime<Utc>,
    pub direction: String,
    pub asset: String,
    pub amount_atomic: String,
    pub usd_value_at_event: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaggedAddressDto {
    pub addr: String,
    pub kind: String,
    pub asset: Option<String>,
    pub label: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletRulesDto {
    pub addresses: Vec<TaggedAddressDto>,
    pub last_loaded: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionInfoDto {
    pub current: Option<String>,
    pub latest: Option<String>,
    pub has_update: bool,
    pub releases_url: Option<String>,
    pub fetch_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PauseStateDto {
    pub is_paused: bool,
    pub since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub ok: bool,
    pub message: Option<String>,
}
