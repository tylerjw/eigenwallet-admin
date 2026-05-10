//! Every 10 minutes: trigger a `list-sellers` Job if one hasn't run recently.

use std::time::Duration;

use crate::server::api::competitors;
use crate::server::state::AppState;

pub async fn run(state: AppState) {
    let mut tick = tokio::time::interval(Duration::from_secs(600));
    loop {
        tick.tick().await;
        if let Err(e) = competitors::trigger(&state.0).await {
            tracing::warn!(error = %e, "competitor scan trigger failed");
        }
    }
}
