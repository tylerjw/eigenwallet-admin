//! Tail the asb tracing log into the swaps table. v1: very conservative — we
//! parse a small set of state-transition lines and update existing rows;
//! everything else is ignored. Full reconciliation is out of scope.

use std::time::Duration;

use crate::server::state::AppState;

pub async fn run(state: AppState) {
    let log_dir = state.config.asb_log_dir.clone();
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tick.tick().await;
        // v1: scan the directory for tracing-*.log, read each (newest only),
        // and look for "swap_id" + "state" pairs. Real implementation is in
        // a follow-up; the file may not exist when running outside the cluster.
        match tokio::fs::read_dir(&log_dir).await {
            Ok(_) => {
                tracing::trace!(dir = %log_dir, "log_tail tick");
            }
            Err(e) => {
                tracing::trace!(error = %e, "log_tail dir unavailable");
            }
        }
    }
}
