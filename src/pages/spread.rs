use leptos::prelude::*;

use crate::types::{
    MakerConfigDto, MakerConfigUpdate, MakerConfigUpdateResult, SpreadOptimizerConfigDto,
    SpreadOptimizerRecommendationDto, SpreadRecommendationDto,
};

#[server(name = GetMakerConfig, prefix = "/api", endpoint = "maker/config/get")]
pub async fn get_maker_config() -> Result<MakerConfigDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::maker::read_config(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = UpdateMakerConfig, prefix = "/api", endpoint = "maker/config/save")]
pub async fn update_maker_config(
    update: MakerConfigUpdate,
) -> Result<MakerConfigUpdateResult, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::maker::write_config(&state, update)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetSpreadRecommendation, prefix = "/api", endpoint = "spread/recommendation")]
pub async fn get_spread_recommendation() -> Result<SpreadRecommendationDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::spread::recommend(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetOptimizerRecommendation, prefix = "/api", endpoint = "spread/optimizer/recommend")]
pub async fn get_optimizer_recommendation()
-> Result<SpreadOptimizerRecommendationDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::spread_optimizer::recommend(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetOptimizerConfig, prefix = "/api", endpoint = "spread/optimizer/config/get")]
pub async fn get_optimizer_config() -> Result<SpreadOptimizerConfigDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::spread_optimizer::get_config(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = SaveOptimizerConfig, prefix = "/api", endpoint = "spread/optimizer/config/save")]
pub async fn save_optimizer_config(cfg: SpreadOptimizerConfigDto) -> Result<(), ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::spread_optimizer::save_config(&state, cfg)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = ApplyOptimizerRec, prefix = "/api", endpoint = "spread/optimizer/apply")]
pub async fn apply_optimizer_recommendation() -> Result<String, ServerFnError> {
    let state = crate::server::ssr_state()?;
    // Run a fresh recommendation, persist it, then apply.
    let rec = crate::server::api::spread_optimizer::recommend(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let id = crate::server::api::spread_optimizer::save_recommendation(&state, &rec)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::server::api::spread_optimizer::apply_recommendation(&state, id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(format!(
        "Applied spread {} (asb will roll within ~30-60 s).",
        rec.recommended_spread
    ))
}

#[component]
pub fn SpreadPage() -> impl IntoView {
    let config = Resource::new(|| (), |_| async move { get_maker_config().await });
    let rec = Resource::new(|| (), |_| async move { get_spread_recommendation().await });
    let opt_reload = RwSignal::new(0i32);
    let opt = Resource::new(
        move || opt_reload.get(),
        |_| async move { get_optimizer_recommendation().await },
    );
    let opt_cfg = Resource::new(|| (), |_| async move { get_optimizer_config().await });

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Spread control"</h1>
            <Suspense fallback=move || view! { <div class="tile text-slate-400">"Loading optimizer…"</div> }>
                {move || opt.get().map(|res| match res {
                    Ok(r) => view! { <OptimizerCard rec=r reload=opt_reload/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || opt_cfg.get().map(|res| match res {
                    Ok(c) => view! { <OptimizerConfigForm config=c/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || rec.get().map(|res| match res {
                    Ok(r) => view! { <RecommendationCard rec=r/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || config.get().map(|res| match res {
                    Ok(c) => view! { <ConfigForm config=c/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn OptimizerCard(rec: SpreadOptimizerRecommendationDto, reload: RwSignal<i32>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let status = RwSignal::new(Option::<String>::None);

    let cur: f64 = rec.current_spread.parse().unwrap_or(0.0);
    let recm: f64 = rec.recommended_spread.parse().unwrap_or(0.0);
    let direction_color = if recm > cur {
        "text-amber-300"
    } else if recm < cur {
        "text-emerald-300"
    } else {
        "text-slate-200"
    };
    let big = format!("{:.3}%", recm * 100.0);
    let cur_str = format!("{:.3}%", cur * 100.0);

    let on_apply = move |_| {
        busy.set(true);
        status.set(None);
        leptos::task::spawn_local(async move {
            match apply_optimizer_recommendation().await {
                Ok(m) => {
                    status.set(Some(m));
                    reload.update(|n| *n += 1);
                }
                Err(e) => status.set(Some(format!("FAIL: {e}"))),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="tile">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
                <div class="tile-title">"Auto-spread optimizer"</div>
                <div class=format!("text-2xl font-semibold {direction_color}")>{big}</div>
            </div>
            <div class="mt-1 text-xs text-slate-400">{format!("current {cur_str}")}</div>
            <div class="mt-2 text-sm text-slate-300">{rec.rationale}</div>
            <details class="mt-3 text-xs text-slate-400">
                <summary class="cursor-pointer">"components"</summary>
                <div class="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 font-mono">
                    <span>"floor:"</span><span>{format_pct(&rec.components.floor)}</span>
                    <span>"vol_term:"</span><span>{format_pct(&rec.components.vol_term)}</span>
                    <span>"inventory_term:"</span><span>{format_pct(&rec.components.inventory_term)}</span>
                    <span>"competitor_term:"</span><span>{format_pct(&rec.components.competitor_term)}</span>
                    <span>"margin_term:"</span><span>{format_pct(&rec.components.margin_term)}</span>
                    <span>"σ_30min:"</span><span>{format_pct(&rec.components.raw_vol_30min)}</span>
                    <span>"inventory skew:"</span><span>{rec.components.inventory_skew}</span>
                    <span>"avg swap $:"</span><span>{rec.components.avg_swap_usd}</span>
                    <span>"tier-1 cutoff:"</span><span>{rec.components.tier1_cutoff_pct.unwrap_or_else(|| "—".into())}</span>
                </div>
            </details>
            <div class="mt-3 flex items-center gap-3">
                <button class="btn" on:click=on_apply prop:disabled=move || busy.get()>
                    {move || if busy.get() { "Applying…" } else { "Apply now (manual)" }}
                </button>
                {move || if rec.auto_apply {
                    view! { <span class="text-xs text-emerald-300">"Auto-apply ON — poller will push this every 15 min if it changes."</span> }.into_any()
                } else {
                    view! { <span class="text-xs text-slate-500">"Auto-apply OFF — recommendation only."</span> }.into_any()
                }}
            </div>
            {move || status.get().map(|s| view! {
                <div class="mt-2 text-xs text-slate-400">{s}</div>
            })}
        </div>
    }
}

fn format_pct(raw: &str) -> String {
    raw.parse::<f64>()
        .map(|v| format!("{:.3}%", v * 100.0))
        .unwrap_or_else(|_| raw.to_string())
}

#[component]
fn OptimizerConfigForm(config: SpreadOptimizerConfigDto) -> impl IntoView {
    let gamma = RwSignal::new(config.gamma.clone());
    let min_spread = RwSignal::new(config.min_spread.clone());
    let max_spread = RwSignal::new(config.max_spread.clone());
    let target_profit = RwSignal::new(config.target_swap_profit_usd.clone());
    let recycle_cost = RwSignal::new(config.amortized_recycle_cost_usd.clone());
    let chain_fees = RwSignal::new(config.chain_fees_per_swap_usd.clone());
    let step_max = RwSignal::new(config.step_size_max.clone());
    let cooldown = RwSignal::new(config.cooldown_seconds.to_string());
    let auto_apply = RwSignal::new(config.auto_apply);
    let status = RwSignal::new(Option::<String>::None);
    let saving = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let cd = cooldown.get().parse::<i32>().unwrap_or(1800);
        let cfg = SpreadOptimizerConfigDto {
            gamma: gamma.get(),
            min_spread: min_spread.get(),
            max_spread: max_spread.get(),
            target_swap_profit_usd: target_profit.get(),
            amortized_recycle_cost_usd: recycle_cost.get(),
            chain_fees_per_swap_usd: chain_fees.get(),
            step_size_max: step_max.get(),
            cooldown_seconds: cd,
            auto_apply: auto_apply.get(),
        };
        saving.set(true);
        leptos::task::spawn_local(async move {
            match save_optimizer_config(cfg).await {
                Ok(()) => status.set(Some("Saved.".into())),
                Err(e) => status.set(Some(format!("FAIL: {e}"))),
            }
            saving.set(false);
        });
    };

    view! {
        <details class="tile">
            <summary class="cursor-pointer text-sm font-medium text-slate-200">"Optimizer settings"</summary>
            <form class="mt-3 space-y-3" on:submit=on_submit>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <Field label="gamma (risk aversion)" sig=gamma/>
                    <Field label="min_spread (fraction)" sig=min_spread/>
                    <Field label="max_spread (fraction)" sig=max_spread/>
                    <Field label="target_swap_profit_usd" sig=target_profit/>
                    <Field label="amortized_recycle_cost_usd" sig=recycle_cost/>
                    <Field label="chain_fees_per_swap_usd" sig=chain_fees/>
                    <Field label="step_size_max (fraction)" sig=step_max/>
                    <Field label="cooldown_seconds" sig=cooldown/>
                </div>
                <label class="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        prop:checked=move || auto_apply.get()
                        on:change=move |ev| auto_apply.set(event_target_checked(&ev))
                    />
                    <span>"Auto-apply recommendations (poller writes to asb every cooldown_seconds)"</span>
                </label>
                <div class="flex items-center gap-3">
                    <button type="submit" class="btn" disabled=move || saving.get()>
                        {move || if saving.get() { "Saving…" } else { "Save settings" }}
                    </button>
                    {move || status.get().map(|s| view! { <span class="text-xs text-slate-400">{s}</span> })}
                </div>
            </form>
        </details>
    }
}

#[component]
fn RecommendationCard(rec: SpreadRecommendationDto) -> impl IntoView {
    view! {
        <div class="tile">
            <div class="tile-title">"Recommendation"</div>
            <div class="mt-1 text-base">{rec.reasoning}</div>
            <div class="mt-2 text-xs text-slate-500">
                {format!(
                    "current {} • recommend {} • tier-1 cutoff {} • rank {}",
                    rec.current_spread_pct.unwrap_or_else(|| "—".into()),
                    rec.recommended_spread_pct.unwrap_or_else(|| "—".into()),
                    rec.tier_1_cutoff_pct.unwrap_or_else(|| "—".into()),
                    rec.our_rank.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                )}
            </div>
        </div>
    }
}

#[component]
fn ConfigForm(config: MakerConfigDto) -> impl IntoView {
    let min_buy = RwSignal::new(config.min_buy_btc.clone());
    let max_buy = RwSignal::new(config.max_buy_btc.clone());
    let spread = RwSignal::new(config.ask_spread.clone());
    let dev_tip = RwSignal::new(config.developer_tip.clone());
    let anti_spam = RwSignal::new(config.anti_spam_deposit_ratio.clone());
    let status = RwSignal::new(Option::<String>::None);
    let saving = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let update = MakerConfigUpdate {
            min_buy_btc: min_buy.get(),
            max_buy_btc: max_buy.get(),
            ask_spread: spread.get(),
            developer_tip: dev_tip.get(),
            anti_spam_deposit_ratio: anti_spam.get(),
        };
        saving.set(true);
        leptos::task::spawn_local(async move {
            match update_maker_config(update).await {
                Ok(r) => status.set(Some(format!("OK: {}", r.message))),
                Err(e) => status.set(Some(format!("FAIL: {e}"))),
            }
            saving.set(false);
        });
    };

    view! {
        <form class="tile space-y-3" on:submit=on_submit>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                <Field label="min_buy_btc" sig=min_buy/>
                <Field label="max_buy_btc" sig=max_buy/>
                <Field label="ask_spread" sig=spread/>
                <Field label="developer_tip" sig=dev_tip/>
                <Field label="anti_spam_deposit_ratio" sig=anti_spam/>
            </div>
            <div class="text-xs text-slate-500">
                "Saving rewrites the ConfigMap and bumps the asb Deployment's config-version annotation, "
                "which triggers a 30-60 s pod restart and the maker is offline during that window."
            </div>
            <div class="flex items-center gap-3">
                <button type="submit" class="btn" disabled=move || saving.get()>
                    {move || if saving.get() { "Saving…" } else { "Save and restart asb" }}
                </button>
                {move || status.get().map(|s| view! { <span class="text-xs text-slate-400">{s}</span> })}
            </div>
        </form>
    }
}

#[component]
fn Field(label: &'static str, sig: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="text-xs uppercase tracking-wide text-slate-400">
            {label}
            <input
                type="text"
                class="input mt-1"
                prop:value=move || sig.get()
                on:input=move |ev| sig.set(event_target_value(&ev))
            />
        </label>
    }
}
