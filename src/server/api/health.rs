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
            detail: Some(state.asb.url().to_string()),
        },
        Ok(false) | Err(_) => SubsystemHealth {
            state: HealthState::Down,
            headline: "unreachable".into(),
            detail: Some(state.asb.url().to_string()),
        },
    }
}

async fn check_bitcoind_via_electrs(state: &AppStateInner) -> SubsystemHealth {
    match state.electrs.tip_height().await {
        Ok(h) => SubsystemHealth {
            state: HealthState::Ok,
            headline: format!("tip {h}"),
            detail: Some("via electrs (bitcoind RPC not exposed)".into()),
        },
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no tip".into(),
            detail: Some(e.to_string()),
        },
    }
}

async fn check_monerod(state: &AppStateInner) -> SubsystemHealth {
    match state.monerod.get_info().await {
        Ok(info) => {
            let synced = info.synchronized;
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
                detail: Some(if synced { "synced" } else { "syncing" }.into()),
            }
        }
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no info".into(),
            detail: Some(e.to_string()),
        },
    }
}

async fn check_electrs(state: &AppStateInner) -> SubsystemHealth {
    match state.electrs.tip_height().await {
        Ok(h) => SubsystemHealth {
            state: HealthState::Ok,
            headline: format!("tip {h}"),
            detail: None,
        },
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no tip".into(),
            detail: Some(e.to_string()),
        },
    }
}

async fn check_tor(state: &AppStateInner) -> SubsystemHealth {
    match state.asb.onion_service_status().await {
        Ok(s) => SubsystemHealth {
            state: if s.reachable {
                HealthState::Ok
            } else if s.state.eq_ignore_ascii_case("Bootstrapping") {
                HealthState::Degraded
            } else {
                HealthState::Down
            },
            headline: s.state.clone(),
            detail: s.problem,
        },
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
                    detail: None,
                }
            }
        }
    }
}

async fn check_peers(state: &AppStateInner) -> SubsystemHealth {
    match state.asb.active_connections().await {
        Ok(n) => SubsystemHealth {
            state: if n > 0 {
                HealthState::Ok
            } else {
                HealthState::Degraded
            },
            headline: format!("{n} peers"),
            detail: None,
        },
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
            SubsystemHealth {
                state: st,
                headline: format!("{registered}/{total} registered"),
                detail: None,
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
                detail: None,
            },
            Err(e) => SubsystemHealth {
                state: HealthState::Down,
                headline: "query failed".into(),
                detail: Some(e.to_string()),
            },
        },
        Err(e) => SubsystemHealth {
            state: HealthState::Down,
            headline: "no connection".into(),
            detail: Some(e.to_string()),
        },
    }
}
