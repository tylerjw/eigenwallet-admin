use leptos::prelude::*;

use crate::types::{
    MakerConfigDto, MakerConfigUpdate, MakerConfigUpdateResult, SpreadRecommendationDto,
};

#[server(name = GetMakerConfig, prefix = "/api", endpoint = "maker/config")]
pub async fn get_maker_config() -> Result<MakerConfigDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::maker::read_config(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = UpdateMakerConfig, prefix = "/api", endpoint = "maker/config")]
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

#[component]
pub fn SpreadPage() -> impl IntoView {
    let config = Resource::new(|| (), |_| async move { get_maker_config().await });
    let rec = Resource::new(|| (), |_| async move { get_spread_recommendation().await });

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Spread control"</h1>
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
