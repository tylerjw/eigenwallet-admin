use anyhow::{Result, anyhow};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::server::db;
use crate::server::models::{
    CompetitorQuote, CompetitorScan, NewCompetitorQuote, NewCompetitorScan,
};
use crate::server::schema::{competitor_quotes, competitor_scans};
use crate::server::state::AppStateInner;
use crate::types::{CompetitorQuoteDto, CompetitorScanDto};

pub async fn latest(state: &AppStateInner) -> Result<Option<CompetitorScanDto>> {
    let mut conn = db::checkout(&state.pool).await?;
    let scan: Option<CompetitorScan> = competitor_scans::table
        .filter(competitor_scans::completed_at.is_not_null())
        .order(competitor_scans::completed_at.desc())
        .first(&mut *conn)
        .await
        .optional()?;
    let Some(scan) = scan else {
        return Ok(None);
    };
    let quotes: Vec<CompetitorQuote> = competitor_quotes::table
        .filter(competitor_quotes::scan_id.eq(scan.scan_id))
        .load(&mut *conn)
        .await?;

    let cex_btc_xmr = state.cex.read().await.last.clone().and_then(|s| s.btc_xmr);

    // Our identity in the competitor list. If asb returned a peer_id, mark
    // any row with that peer_id as `is_us` AND add our own quote as a
    // synthetic row at the top so the operator can see where they sit
    // alongside the field. The synthetic row is only added if the scan
    // doesn't already contain us — `list-sellers` from the cluster usually
    // excludes us, but we don't rely on that.
    let our_peer_id = state.asb.peer_id().await.ok();
    let our_quote = state.asb.get_current_quote().await.ok();
    const SATS_PER_BTC: i64 = 100_000_000;

    let mut quotes: Vec<CompetitorQuoteDto> = quotes
        .into_iter()
        .map(|q| {
            let spread_pct = match (q.price_btc_per_xmr, cex_btc_xmr) {
                (Some(p), Some(mid)) if !mid.is_zero() => {
                    let pct = (p - mid) / mid * Decimal::from(100);
                    Some(pct.round_dp(2).to_string())
                }
                _ => None,
            };
            let is_us = our_peer_id
                .as_deref()
                .map(|us| us == q.peer_id)
                .unwrap_or(false);
            CompetitorQuoteDto {
                peer_id: q.peer_id,
                multiaddr: q.multiaddr,
                price_btc_per_xmr: q.price_btc_per_xmr.map(|d| d.to_string()),
                min_btc: q.min_btc.map(|d| d.to_string()),
                max_btc: q.max_btc.map(|d| d.to_string()),
                reachable: q.reachable,
                reason_if_unreachable: q.reason_if_unreachable,
                spread_vs_cex_pct: spread_pct,
                is_us,
                version: q.version,
            }
        })
        .collect();

    let scan_includes_us = quotes.iter().any(|q| q.is_us);
    if !scan_includes_us && let (Some(pid), Some(q)) = (our_peer_id.clone(), our_quote) {
        {
            let sats = Decimal::from(SATS_PER_BTC);
            let price_btc = Decimal::from(q.price) / sats;
            let min_btc = Decimal::from(q.min_quantity) / sats;
            let max_btc = Decimal::from(q.max_quantity) / sats;
            let spread = cex_btc_xmr.filter(|m| !m.is_zero()).map(|m| {
                ((price_btc - m) / m * Decimal::from(100))
                    .round_dp(2)
                    .to_string()
            });
            quotes.insert(
                0,
                CompetitorQuoteDto {
                    peer_id: pid,
                    multiaddr: None,
                    price_btc_per_xmr: Some(price_btc.to_string()),
                    min_btc: Some(min_btc.to_string()),
                    max_btc: Some(max_btc.to_string()),
                    reachable: true,
                    reason_if_unreachable: None,
                    spread_vs_cex_pct: spread,
                    is_us: true,
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                },
            );
        }
    }

    // Sort by price ascending — cheapest XMR for the taker comes first, which
    // is the natural reading order ("how does my quote compare"). Entries
    // without a price ("no quote") sink to the bottom.
    quotes.sort_by(|a, b| {
        let pa = a
            .price_btc_per_xmr
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::INFINITY);
        let pb = b
            .price_btc_per_xmr
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::INFINITY);
        pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Some(CompetitorScanDto {
        scan_id: scan.scan_id.to_string(),
        started_at: scan.started_at,
        completed_at: scan.completed_at,
        trigger: scan.trigger,
        quotes,
    }))
}

/// Kick off a list-sellers scan. Polls the resulting Job to completion, parses
/// the JSON output, and writes a scan + quotes rows. Takes the shared Arc so
/// the spawned watcher can outlive the caller.
pub async fn trigger(state: &std::sync::Arc<AppStateInner>) -> Result<String> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube unavailable"))?;
    let scan_id = Uuid::new_v4();
    {
        let mut conn = db::checkout(&state.pool).await?;
        diesel::insert_into(competitor_scans::table)
            .values(NewCompetitorScan {
                scan_id,
                started_at: Utc::now(),
                trigger: "manual".into(),
            })
            .execute(&mut *conn)
            .await?;
    }

    let job = kube.build_scan_job(
        &state.config.asb_namespace,
        &state.config.swap_cli_image,
        &state.config.rendezvous_points,
        state.config.our_peer_id.as_deref(),
    );
    let created = kube.create_job(&state.config.asb_namespace, &job).await?;
    let job_name = created
        .metadata
        .name
        .clone()
        .ok_or_else(|| anyhow!("created job has no name"))?;

    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = await_scan(state_clone, scan_id, job_name).await {
            tracing::warn!(error = %e, "scan await failed");
        }
    });

    Ok(scan_id.to_string())
}

async fn await_scan(
    state: std::sync::Arc<AppStateInner>,
    scan_id: Uuid,
    job_name: String,
) -> Result<()> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube unavailable"))?;
    let ns = state.config.asb_namespace.clone();
    for _ in 0..120 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let job = kube.get_job(&ns, &job_name).await?;
        let status = job.status.clone().unwrap_or_default();
        if status.succeeded.unwrap_or(0) > 0 || status.failed.unwrap_or(0) > 0 {
            let logs = kube.job_pod_logs(&ns, &job_name).await.unwrap_or_default();
            parse_and_persist(&state, scan_id, &logs).await?;
            let mut conn = db::checkout(&state.pool).await?;
            diesel::update(competitor_scans::table.filter(competitor_scans::scan_id.eq(scan_id)))
                .set((
                    competitor_scans::completed_at.eq(Some(Utc::now())),
                    competitor_scans::raw_output.eq(Some(logs)),
                ))
                .execute(&mut *conn)
                .await?;
            return Ok(());
        }
    }
    Err(anyhow!("scan job {job_name} did not finish in time"))
}

/// Parse the swap-cli `list-sellers` output. The binary writes a stream of
/// tracing logs first, then a single pretty-printed JSON array (one `[` on its
/// own line, sellers, then `]`) to stdout. We locate the last top-level `[`
/// line and parse from there to end of input.
///
/// Each seller entry has the shape (verified against asb 4.5.0 on 2026-05-10):
///   { multiaddr, peer_id, quote: { price, min_quantity, max_quantity, ... }, version }
/// Amounts are integers in satoshi. `price` is satoshi per 1 XMR.
async fn parse_and_persist(state: &AppStateInner, scan_id: Uuid, logs: &str) -> Result<()> {
    const SATS_PER_BTC: i64 = 100_000_000;

    let lines: Vec<&str> = logs.lines().collect();
    let start_line = lines.iter().rposition(|l| l.trim() == "[");
    let Some(start_line) = start_line else {
        tracing::warn!("scan output: no `[` line found, skipping persist");
        return Ok(());
    };
    let body: String = lines[start_line..].join("\n");
    let v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "scan output JSON parse failed");
            return Ok(());
        }
    };
    let Some(sellers) = v.as_array() else {
        return Ok(());
    };

    let sats = Decimal::from(SATS_PER_BTC);
    let mut quotes: Vec<NewCompetitorQuote> = Vec::new();
    for s in sellers {
        let Some(peer_id) = s.get("peer_id").and_then(|p| p.as_str()) else {
            continue;
        };
        let multiaddr = s
            .get("multiaddr")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let quote = s.get("quote");
        let price_sat = quote.and_then(|q| q.get("price")).and_then(|p| p.as_i64());
        let min_sat = quote
            .and_then(|q| q.get("min_quantity"))
            .and_then(|p| p.as_i64());
        let max_sat = quote
            .and_then(|q| q.get("max_quantity"))
            .and_then(|p| p.as_i64());
        // A seller with all-zero amounts is effectively unreachable / disabled.
        let reachable = quote.is_some() && price_sat.unwrap_or(0) > 0 && max_sat.unwrap_or(0) > 0;
        let version = s
            .get("version")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        quotes.push(NewCompetitorQuote {
            scan_id,
            peer_id: peer_id.to_string(),
            multiaddr,
            price_btc_per_xmr: price_sat.map(|p| Decimal::from(p) / sats),
            min_btc: min_sat.map(|m| Decimal::from(m) / sats),
            max_btc: max_sat.map(|m| Decimal::from(m) / sats),
            reachable,
            reason_if_unreachable: (!reachable).then(|| "no quote".to_string()),
            version,
        });
    }
    if quotes.is_empty() {
        return Ok(());
    }
    let mut conn = db::checkout(&state.pool).await?;
    diesel::insert_into(competitor_quotes::table)
        .values(&quotes)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
