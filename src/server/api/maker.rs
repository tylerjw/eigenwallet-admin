//! Read/write the [maker] section of the asb-config ConfigMap, and bump the
//! Deployment's config-version annotation to trigger a rolling restart.

use anyhow::{Result, anyhow};
use diesel_async::RunQueryDsl;
use toml_edit::{DocumentMut, value};

use crate::server::db;
use crate::server::models::NewMakerConfigHistory;
use crate::server::schema::maker_config_history;
use crate::server::state::AppStateInner;
use crate::types::{MakerConfigDto, MakerConfigUpdate, MakerConfigUpdateResult};

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
