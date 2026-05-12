use anyhow::Result;
use diesel::sql_query;
use diesel_async::RunQueryDsl;

use crate::server::db;
use crate::server::state::AppStateInner;
use crate::types::{HealthDto, HealthState, SubsystemHealth};

pub async fn fetch(state: &AppStateInner) -> Result<HealthDto> {
    let asb = check_asb(state).await;
    let bitcoind = check_bitcoind_via_electrs(state).await;
    let monerod = check_monerod(state).await;
    let electrs = check_electrs(state).await;
    let tor = check_tor(state).await;
    let peers = check_peers(state).await;
    let rendezvous = check_rendezvous(state).await;
    let admin_db = check_admin_db(state).await;

    Ok(HealthDto {
        asb,
        bitcoind,
        monerod,
        electrs,
        tor,
        peers,
        rendezvous,
        admin_db,
        as_of: chrono::Utc::now(),
    })
}

async fn check_asb(state: &AppStateInner) -> SubsystemHealth {
    match state.asb.check_connection().await {
        Ok(true) => SubsystemHealth {
            state: HealthState::Ok,
            headline: "reachable".into(),
            detail: Some(format!("RPC: {}", state.asb.url())),
        },
        Ok(false) | Err(_) => SubsystemHealth {
            state: HealthState::Down,
            headline: "unreachable".into(),
            detail: Some(format!(
                "Can't reach asb at {}. If we just bumped the [maker] config this is normal for ~30-60s while the pod rolls. Otherwise: `kubectl get pod -n eigenwallet -l app=asb` and `kubectl logs -n eigenwallet deploy/asb`.",
                state.asb.url()
            )),
        },
    }
}

async fn check_bitcoind_via_electrs(state: &AppStateInner) -> SubsystemHealth {
    match state.electrs.tip_height().await {
        Ok(h) => SubsystemHealth {
            state: HealthState::Ok,
            headline: format!("tip {h}"),
            detail: Some(
                "Bitcoin tip looks good. (Reported via electrs since bitcoind's RPC isn't exposed.)".into(),
            ),
        },
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no tip".into(),
            detail: Some(format!(
                "Couldn't reach electrs to ask for the Bitcoin tip. Check `kubectl get pod -n eigenwallet -l app=electrs` and `kubectl logs -n eigenwallet deploy/electrs`. Underlying error: {e}"
            )),
        },
    }
}

async fn check_monerod(state: &AppStateInner) -> SubsystemHealth {
    match state.monerod.get_info().await {
        Ok(info) => {
            let synced = info.synchronized;
            let detail = if synced {
                Some("synced — XMR side ready".to_string())
            } else {
                Some(
                    "monerod is still syncing the Monero chain. Swaps can't lock XMR until this catches up; just wait. Progress: `kubectl logs -n eigenwallet deploy/monerod -f`."
                        .to_string(),
                )
            };
            SubsystemHealth {
                state: if synced {
                    HealthState::Ok
                } else {
                    HealthState::Degraded
                },
                headline: format!(
                    "height {} / {}",
                    info.height,
                    info.target_height.max(info.height)
                ),
                detail,
            }
        }
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no info".into(),
            detail: Some(format!(
                "Couldn't query monerod RPC. Check pod state: `kubectl get pod -n eigenwallet -l app=monerod` and logs. Underlying error: {e}"
            )),
        },
    }
}

async fn check_electrs(state: &AppStateInner) -> SubsystemHealth {
    match state.electrs.tip_height().await {
        Ok(h) => SubsystemHealth {
            state: HealthState::Ok,
            headline: format!("tip {h}"),
            detail: Some("Electrum index reachable; asb can read BTC chain state.".into()),
        },
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no tip".into(),
            detail: Some(format!(
                "Couldn't connect to electrs on tcp/50001. If electrs was just restarted it can take ~hours to reindex. Check `kubectl get pod -n eigenwallet -l app=electrs`. Underlying error: {e}"
            )),
        },
    }
}

async fn check_tor(state: &AppStateInner) -> SubsystemHealth {
    match state.asb.onion_service_status().await {
        Ok(s) => {
            let health_state = if s.reachable {
                HealthState::Ok
            } else if s.state.eq_ignore_ascii_case("Bootstrapping") {
                HealthState::Degraded
            } else {
                HealthState::Down
            };
            let hint = match s.state.as_str() {
                "Bootstrapping" => Some(
                    "Tor circuit is still being built. Usually clears in <5 min after a pod start. Wait it out; if it persists check `kubectl logs -n eigenwallet deploy/tor`."
                        .to_string(),
                ),
                "DegradedUnreachable" => Some(
                    "Tor failed to publish the hidden-service descriptor. Often a transient network issue. If it lingers, restart asb: `kubectl rollout restart deploy/asb -n eigenwallet`."
                        .to_string(),
                ),
                _ => s.problem.clone(),
            };
            SubsystemHealth {
                state: health_state,
                headline: s.state,
                detail: hint,
            }
        }
        Err(_) => {
            // Fall back to checking multiaddresses contain .onion.
            let m = state.asb.multiaddresses().await.unwrap_or_default();
            let onion = m.iter().filter(|s| s.contains(".onion")).count();
            if onion > 0 {
                SubsystemHealth {
                    state: HealthState::Ok,
                    headline: format!("{onion} onion addr"),
                    detail: None,
                }
            } else {
                SubsystemHealth {
                    state: HealthState::Degraded,
                    headline: "no onion".into(),
                    detail: Some(
                        "asb hasn't published its hidden-service address. Check tor: `kubectl logs -n eigenwallet deploy/tor`."
                            .to_string(),
                    ),
                }
            }
        }
    }
}

async fn check_peers(state: &AppStateInner) -> SubsystemHealth {
    match state.asb.active_connections().await {
        Ok(n) => {
            let detail = if n == 0 {
                Some(
                    "No peers connected. We can't trade until at least one taker finds us. \
                     Confirm Tor and rendezvous are up (tiles above)."
                        .to_string(),
                )
            } else {
                None
            };
            SubsystemHealth {
                state: if n > 0 {
                    HealthState::Ok
                } else {
                    HealthState::Degraded
                },
                headline: format!("{n} peers"),
                detail,
            }
        }
        Err(e) => SubsystemHealth {
            state: HealthState::Unknown,
            headline: "unknown".into(),
            detail: Some(e.to_string()),
        },
    }
}

async fn check_rendezvous(state: &AppStateInner) -> SubsystemHealth {
    match state.asb.registration_status().await {
        Ok(r) => {
            let registered = r.registered_count();
            let total = r.total();
            let st = if total == 0 {
                HealthState::Unknown
            } else if registered >= total {
                HealthState::Ok
            } else if registered > 0 {
                HealthState::Degraded
            } else {
                HealthState::Down
            };
            let unreg = total - registered;
            let detail = if st == HealthState::Ok {
                None
            } else if registered > 0 {
                Some(format!(
                    "{unreg} of {total} rendezvous nodes are unreachable or haven't registered yet. \
                     Other makers can still discover us via the {registered} that are up, so this is \
                     usually fine. The unreachable ones are 3rd-party servers — nothing actionable here \
                     beyond waiting for them to come back."
                ))
            } else {
                Some(
                    "No rendezvous registrations. Either the rendezvous list is empty in the asb \
                     config or Tor isn't routing. Check the asb-config ConfigMap's `rendezvous_point` \
                     list and the tor health tile above."
                        .to_string(),
                )
            };
            SubsystemHealth {
                state: st,
                headline: format!("{registered}/{total} registered"),
                detail,
            }
        }
        Err(e) => SubsystemHealth {
            state: HealthState::Unknown,
            headline: "unknown".into(),
            detail: Some(e.to_string()),
        },
    }
}

async fn check_admin_db(state: &AppStateInner) -> SubsystemHealth {
    match db::checkout(&state.pool).await {
        Ok(mut conn) => match sql_query("SELECT 1").execute(&mut *conn).await {
            Ok(_) => SubsystemHealth {
                state: HealthState::Ok,
                headline: "reachable".into(),
                detail: Some("Postgres for snapshots / capital events / scans.".into()),
            },
            Err(e) => SubsystemHealth {
                state: HealthState::Down,
                headline: "query failed".into(),
                detail: Some(format!(
                    "Postgres is up but a SELECT 1 failed. This is rare — check `kubectl get cluster admin-db -n eigenwallet` and CNPG operator logs. Underlying error: {e}"
                )),
            },
        },
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no connection".into(),
            detail: Some(format!(
                "Can't get a connection from the pool. The admin-db pod may be down or starting. `kubectl get cluster admin-db -n eigenwallet`. Underlying error: {e}"
            )),
        },
    }
}
