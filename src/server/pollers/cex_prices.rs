//! Every 15s: refresh in-memory CEX cache and (every minute) persist a row.

use std::time::Duration;

use chrono::Utc;
use diesel_async::RunQueryDsl;
use reqwest::Client;

use crate::server::clients::cex;
use crate::server::db;
use crate::server::models::CexPrice;
use crate::server::schema::cex_prices;
use crate::server::state::AppState;

pub async fn run(state: AppState) {
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .expect("cex client");
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    let mut last_persisted = std::time::Instant::now() - Duration::from_secs(120);
    loop {
        tick.tick().await;
        let snap = cex::fetch_all(&client).await;
        {
            let mut cache = state.cex.write().await;
            cache.last = Some(snap.clone());
            cache.fetched_at = Some(std::time::Instant::now());
        }
        if last_persisted.elapsed() >= Duration::from_secs(60) {
            last_persisted = std::time::Instant::now();
            if let Err(e) = persist(&state, &snap).await {
                tracing::warn!(error = %e, "cex price persist failed");
            }
        }
    }
}

async fn persist(state: &AppState, snap: &cex::CexSnapshot) -> anyhow::Result<()> {
    let mut conn = db::checkout(&state.pool).await?;
    let row = CexPrice {
        sampled_at: snap.sampled_at.with_timezone(&Utc),
        btc_usd: snap.btc_usd,
        xmr_usd: snap.xmr_usd,
        btc_xmr: snap.btc_xmr,
        sources: snap.sources.clone(),
    };
    diesel::insert_into(cex_prices::table)
        .values(&row)
        .on_conflict(cex_prices::sampled_at)
        .do_nothing()
        .execute(&mut *conn)
        .await?;
    Ok(())
}
