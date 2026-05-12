//! Read/write the [maker] section of the asb-config ConfigMap, and bump the
//! Deployment's config-version annotation to trigger a rolling restart.

use anyhow::{Result, anyhow};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use toml_edit::{DocumentMut, value};

use crate::server::db;
use crate::server::models::{MakerConfigHistory, NewMakerConfigHistory};
use crate::server::schema::maker_config_history;
use crate::server::state::AppStateInner;
use crate::types::{MakerConfigDto, MakerConfigUpdate, MakerConfigUpdateResult, PauseStateDto};

/// Marker stored in `maker_config_history.notes` to identify pause/resume
/// transitions distinct from regular [maker] edits.
const PAUSE_MARKER: &str = "paused-by-admin";
const RESUME_MARKER: &str = "resumed-by-admin";

/// "Off-market" values written to [maker] on pause. ask_spread=5.0 is a 500%
/// premium so no rational taker will swap with us; max_buy_btc=0 belts-and-
/// braces makes the quote zero-sized as well.
const PAUSE_ASK_SPREAD: f64 = 5.0;
const PAUSE_MAX_BUY_BTC: f64 = 0.0;

pub async fn read_config(state: &AppStateInner) -> Result<MakerConfigDto> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube client unavailable"))?;
    let cm = kube
        .read_configmap(
            &state.config.asb_namespace,
            &state.config.asb_configmap_name,
        )
        .await?;
    let raw = cm
        .data
        .as_ref()
        .and_then(|m| m.get("config.toml").cloned())
        .ok_or_else(|| anyhow!("config.toml not in ConfigMap"))?;
    let doc = raw.parse::<DocumentMut>()?;
    let maker = doc
        .get("maker")
        .and_then(|i| i.as_table())
        .ok_or_else(|| anyhow!("no [maker] section"))?;
    let refund = doc
        .get("maker")
        .and_then(|i| i.as_table())
        .and_then(|t| t.get("refund_policy"))
        .and_then(|i| i.as_table());

    let s = |k: &str| {
        maker
            .get(k)
            .map(|v| v.to_string().trim().trim_matches('"').to_string())
            .unwrap_or_default()
    };
    let anti = refund
        .and_then(|t| t.get("anti_spam_deposit_ratio"))
        .map(|v| v.to_string().trim().trim_matches('"').to_string())
        .unwrap_or_default();

    Ok(MakerConfigDto {
        min_buy_btc: s("min_buy_btc"),
        max_buy_btc: s("max_buy_btc"),
        ask_spread: s("ask_spread"),
        developer_tip: s("developer_tip"),
        anti_spam_deposit_ratio: anti,
        raw_toml: raw,
    })
}

pub async fn write_config(
    state: &AppStateInner,
    update: MakerConfigUpdate,
) -> Result<MakerConfigUpdateResult> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube client unavailable"))?;
    let ns = &state.config.asb_namespace;
    let cm_name = &state.config.asb_configmap_name;
    let dep_name = &state.config.asb_deployment_name;

    let cm = kube.read_configmap(ns, cm_name).await?;
    let prev = cm
        .data
        .as_ref()
        .and_then(|m| m.get("config.toml").cloned())
        .ok_or_else(|| anyhow!("config.toml not in ConfigMap"))?;

    let mut doc = prev.parse::<DocumentMut>()?;

    let maker = doc
        .get_mut("maker")
        .ok_or_else(|| anyhow!("no [maker] section"))?
        .as_table_mut()
        .ok_or_else(|| anyhow!("[maker] not a table"))?;
    set_decimal(maker, "min_buy_btc", &update.min_buy_btc)?;
    set_decimal(maker, "max_buy_btc", &update.max_buy_btc)?;
    set_decimal(maker, "ask_spread", &update.ask_spread)?;
    set_decimal(maker, "developer_tip", &update.developer_tip)?;
    if let Some(refund) = maker
        .get_mut("refund_policy")
        .and_then(|i| i.as_table_mut())
    {
        set_decimal(
            refund,
            "anti_spam_deposit_ratio",
            &update.anti_spam_deposit_ratio,
        )?;
    }

    let new = doc.to_string();
    kube.write_configmap_data(ns, cm_name, "config.toml", &new)
        .await?;

    // Bump deployment annotation to force rolling restart.
    let stamp = chrono::Utc::now().timestamp().to_string();
    kube.bump_deployment_annotation(ns, dep_name, "config-version", &stamp)
        .await?;

    // Audit log
    let mut conn = db::checkout(&state.pool).await?;
    diesel::insert_into(maker_config_history::table)
        .values(NewMakerConfigHistory {
            previous_toml: prev,
            new_toml: new,
            notes: None,
        })
        .execute(&mut *conn)
        .await?;

    Ok(MakerConfigUpdateResult {
        config_version: stamp,
        message: "ConfigMap updated; asb pod will roll within ~30-60s".into(),
    })
}

fn set_decimal(t: &mut toml_edit::Table, key: &str, raw: &str) -> Result<()> {
    let raw = raw.trim();
    let parsed: f64 = raw
        .parse()
        .map_err(|_| anyhow!("'{key}' not a number: {raw}"))?;
    t[key] = value(parsed);
    Ok(())
}

/// Whether the maker is currently in "paused" state. Derived from the most
/// recent `maker_config_history` row that's tagged with the pause/resume
/// marker. Untagged rows (regular config edits while paused) don't change
/// the state.
pub async fn get_pause_state(state: &AppStateInner) -> Result<PauseStateDto> {
    let mut conn = db::checkout(&state.pool).await?;
    let latest: Option<MakerConfigHistory> = maker_config_history::table
        .filter(
            maker_config_history::notes
                .eq(PAUSE_MARKER)
                .or(maker_config_history::notes.eq(RESUME_MARKER)),
        )
        .order(maker_config_history::changed_at.desc())
        .first::<MakerConfigHistory>(&mut *conn)
        .await
        .optional()?;
    Ok(match latest {
        Some(row) if row.notes.as_deref() == Some(PAUSE_MARKER) => PauseStateDto {
            is_paused: true,
            since: Some(row.changed_at),
        },
        _ => PauseStateDto {
            is_paused: false,
            since: None,
        },
    })
}

/// Pause the maker. Stashes the current [maker] config as `previous_toml`
/// on the history row (so resume can restore exact values), writes an
/// off-market config to the ConfigMap, and bumps the Deployment annotation
/// so asb rolls onto the new config.
pub async fn pause(state: &AppStateInner) -> Result<MakerConfigUpdateResult> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube client unavailable"))?;
    let ns = &state.config.asb_namespace;
    let cm_name = &state.config.asb_configmap_name;
    let dep_name = &state.config.asb_deployment_name;

    if get_pause_state(state).await?.is_paused {
        return Err(anyhow!("maker is already paused"));
    }

    let cm = kube.read_configmap(ns, cm_name).await?;
    let prev = cm
        .data
        .as_ref()
        .and_then(|m| m.get("config.toml").cloned())
        .ok_or_else(|| anyhow!("config.toml not in ConfigMap"))?;

    let mut doc = prev.parse::<DocumentMut>()?;
    let maker = doc
        .get_mut("maker")
        .ok_or_else(|| anyhow!("no [maker] section"))?
        .as_table_mut()
        .ok_or_else(|| anyhow!("[maker] not a table"))?;
    maker["ask_spread"] = value(PAUSE_ASK_SPREAD);
    maker["max_buy_btc"] = value(PAUSE_MAX_BUY_BTC);
    let new = doc.to_string();

    kube.write_configmap_data(ns, cm_name, "config.toml", &new)
        .await?;
    let stamp = chrono::Utc::now().timestamp().to_string();
    kube.bump_deployment_annotation(ns, dep_name, "config-version", &stamp)
        .await?;

    let mut conn = db::checkout(&state.pool).await?;
    diesel::insert_into(maker_config_history::table)
        .values(NewMakerConfigHistory {
            previous_toml: prev,
            new_toml: new,
            notes: Some(PAUSE_MARKER.to_string()),
        })
        .execute(&mut *conn)
        .await?;

    Ok(MakerConfigUpdateResult {
        config_version: stamp,
        message:
            "Maker paused. asb will roll within ~30-60 s and quote off-market until you resume."
                .into(),
    })
}

/// Resume the maker by restoring the [maker] config that was active before
/// the most recent pause. In-flight swaps are unaffected (they're past the
/// quote-accept phase); only new takers will start finding viable quotes.
pub async fn resume(state: &AppStateInner) -> Result<MakerConfigUpdateResult> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube client unavailable"))?;
    let ns = &state.config.asb_namespace;
    let cm_name = &state.config.asb_configmap_name;
    let dep_name = &state.config.asb_deployment_name;

    // The most recent pause-tagged history row is what we restore from.
    let pause_row = {
        let mut conn = db::checkout(&state.pool).await?;
        maker_config_history::table
            .filter(maker_config_history::notes.eq(PAUSE_MARKER))
            .order(maker_config_history::changed_at.desc())
            .first::<MakerConfigHistory>(&mut *conn)
            .await
            .optional()?
            .ok_or_else(|| anyhow!("no pause record found — nothing to resume"))?
    };

    // Defensive: confirm we actually are paused right now. If a later
    // resume row exists, get_pause_state returned false and we'd never
    // reach here from the UI — but a CLI caller could.
    if !get_pause_state(state).await?.is_paused {
        return Err(anyhow!("maker is not paused"));
    }

    // Restore from the pre-pause TOML verbatim — preserves operator-set
    // values exactly, no parse-and-re-emit round trip.
    let restored = pause_row.previous_toml.clone();

    // Read current to populate the audit row's previous_toml.
    let cm = kube.read_configmap(ns, cm_name).await?;
    let current = cm
        .data
        .as_ref()
        .and_then(|m| m.get("config.toml").cloned())
        .ok_or_else(|| anyhow!("config.toml not in ConfigMap"))?;

    kube.write_configmap_data(ns, cm_name, "config.toml", &restored)
        .await?;
    let stamp = chrono::Utc::now().timestamp().to_string();
    kube.bump_deployment_annotation(ns, dep_name, "config-version", &stamp)
        .await?;

    let mut conn = db::checkout(&state.pool).await?;
    diesel::insert_into(maker_config_history::table)
        .values(NewMakerConfigHistory {
            previous_toml: current,
            new_toml: restored,
            notes: Some(RESUME_MARKER.to_string()),
        })
        .execute(&mut *conn)
        .await?;

    Ok(MakerConfigUpdateResult {
        config_version: stamp,
        message: "Maker resumed. asb will roll within ~30-60 s and start quoting again.".into(),
    })
}
