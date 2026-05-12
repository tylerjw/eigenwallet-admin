use leptos::prelude::*;

use crate::components::chart::InteractiveLineChart;
use crate::components::tile::Tile;
use crate::pages::charts::get_account_value;
use crate::types::{
    ChartSeries, MakerConfigUpdateResult, OverviewDto, PauseStateDto, VersionInfoDto,
};

#[server(name = GetOverview, prefix = "/api", endpoint = "overview")]
pub async fn get_overview() -> Result<OverviewDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::overview::fetch(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetVersionInfo, prefix = "/api", endpoint = "version")]
pub async fn get_version_info() -> Result<VersionInfoDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::version::fetch(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetPauseState, prefix = "/api", endpoint = "maker/pause/get")]
pub async fn get_pause_state() -> Result<PauseStateDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::maker::get_pause_state(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = PauseMaker, prefix = "/api", endpoint = "maker/pause")]
pub async fn pause_maker() -> Result<MakerConfigUpdateResult, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::maker::pause(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = ResumeMaker, prefix = "/api", endpoint = "maker/resume")]
pub async fn resume_maker() -> Result<MakerConfigUpdateResult, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::maker::resume(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn OverviewPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { get_overview().await });
    let version = Resource::new(|| (), |_| async move { get_version_info().await });
    let value = Resource::new(
        || (),
        |_| async move { get_account_value(Some("7d".into()), Some("usd".into())).await },
    );
    let pause_reload = RwSignal::new(0i32);
    let pause = Resource::new(
        move || pause_reload.get(),
        |_| async move { get_pause_state().await },
    );

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Overview"</h1>
            <Suspense fallback=move || view! { <VersionBannerLoading/> }>
                {move || version.get().map(|res| match res {
                    Ok(v) => view! { <VersionBanner info=v/> }.into_any(),
                    Err(e) => view! { <VersionBannerError msg=e.to_string()/> }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <div class="text-slate-500 text-sm">"Checking maker state…"</div> }>
                {move || pause.get().map(|res| match res {
                    Ok(s) => view! { <PauseBanner state=s reload=pause_reload/> }.into_any(),
                    Err(e) => view! {
                        <div class="tile border-amber-700 text-amber-300 text-sm">
                            {format!("Pause state unavailable: {e}")}
                        </div>
                    }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <Loading/> }>
                {move || match data.get() {
                    None => view! { <Loading/> }.into_any(),
                    Some(Err(e)) => view! { <ErrorBox msg=e.to_string()/> }.into_any(),
                    Some(Ok(d)) => view! { <OverviewBody data=d/> }.into_any(),
                }}
            </Suspense>
            <div class="tile">
                <div class="tile-title">"Total value (USD, 7d)"</div>
                <Suspense fallback=move || view! { <div class="h-32 text-slate-400">"Loading…"</div> }>
                    {move || value.get().map(|res| match res {
                        Ok(s) => view! { <ValueSparkline series=s/> }.into_any(),
                        Err(e) => view! { <div class="text-rose-300">{e.to_string()}</div> }.into_any(),
                    })}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn PauseBanner(state: PauseStateDto, reload: RwSignal<i32>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<String>::None);
    let err = RwSignal::new(Option::<String>::None);

    let on_pause = move |_| {
        if !web_sys::window()
            .and_then(|w| w.confirm_with_message(
                "Pause the maker?\n\nasb will quote off-market and stop accepting new swaps in ~30-60 s. In-flight swaps continue to settle.",
            ).ok())
            .unwrap_or(false)
        {
            return;
        }
        busy.set(true);
        msg.set(None);
        err.set(None);
        leptos::task::spawn_local(async move {
            match pause_maker().await {
                Ok(r) => {
                    msg.set(Some(r.message));
                    reload.update(|n| *n += 1);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_resume = move |_| {
        busy.set(true);
        msg.set(None);
        err.set(None);
        leptos::task::spawn_local(async move {
            match resume_maker().await {
                Ok(r) => {
                    msg.set(Some(r.message));
                    reload.update(|n| *n += 1);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    if state.is_paused {
        let since = state
            .since
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "unknown".into());
        view! {
            <div class="tile border-rose-700 bg-rose-950/40">
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                        <div class="text-sm font-semibold text-rose-200">
                            "Maker is paused"
                        </div>
                        <div class="text-xs text-rose-300/80">
                            {format!("No new swaps will be accepted. Paused since {since}. In-flight swaps are unaffected.")}
                        </div>
                    </div>
                    <button
                        class="px-3 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-sm font-medium text-white"
                        on:click=on_resume
                        prop:disabled=move || busy.get()
                    >
                        {move || if busy.get() { "Resuming…" } else { "Resume maker" }}
                    </button>
                </div>
                {move || msg.get().map(|m| view! { <div class="mt-2 text-xs text-emerald-300">{m}</div> })}
                {move || err.get().map(|e| view! { <div class="mt-2 text-xs text-rose-300">{e}</div> })}
            </div>
        }
        .into_any()
    } else {
        view! {
            <div class="tile">
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                        <div class="text-sm font-medium text-slate-200">
                            "Maker is live — quoting and accepting swaps"
                        </div>
                        <div class="text-xs text-slate-400">
                            "Pause to stop accepting new swaps (in-flight ones keep settling). Quotes go off-market until you resume."
                        </div>
                    </div>
                    <button
                        class="px-3 py-1.5 rounded bg-rose-700 hover:bg-rose-600 disabled:opacity-50 text-sm font-medium text-white"
                        on:click=on_pause
                        prop:disabled=move || busy.get()
                    >
                        {move || if busy.get() { "Pausing…" } else { "Pause maker" }}
                    </button>
                </div>
                {move || msg.get().map(|m| view! { <div class="mt-2 text-xs text-emerald-300">{m}</div> })}
                {move || err.get().map(|e| view! { <div class="mt-2 text-xs text-rose-300">{e}</div> })}
            </div>
        }
        .into_any()
    }
}

#[component]
fn VersionBanner(info: VersionInfoDto) -> impl IntoView {
    let current_label = info
        .current
        .clone()
        .map(|v| format!("Eigenwallet v{v}"))
        .unwrap_or_else(|| "Eigenwallet — version unknown".to_string());
    let releases_url = info.releases_url.clone();
    let latest = info.latest.clone();
    let has_update = info.has_update;
    let fetch_error = info.fetch_error.clone();

    view! {
        <div class="tile">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="text-sm font-medium text-slate-200">{current_label}</div>
                {move || {
                    if has_update {
                        let url = releases_url.clone().unwrap_or_else(|| {
                            "https://github.com/eigenwallet/eigenwallet/releases".into()
                        });
                        let label = format!(
                            "↑ v{} available — view changelog",
                            latest.clone().unwrap_or_default()
                        );
                        view! {
                            <a
                                class="text-sm text-emerald-300 hover:text-emerald-200 underline"
                                href=url
                                target="_blank"
                                rel="noopener noreferrer"
                            >
                                {label}
                            </a>
                        }
                        .into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }
                }}
            </div>
            {fetch_error.map(|e| view! {
                <div class="mt-1 text-xs text-amber-400">{e}</div>
            })}
        </div>
    }
}

#[component]
fn VersionBannerLoading() -> impl IntoView {
    view! {
        <div class="tile">
            <div class="text-sm text-slate-400">"Eigenwallet — checking version…"</div>
        </div>
    }
}

#[component]
fn VersionBannerError(msg: String) -> impl IntoView {
    view! {
        <div class="tile">
            <div class="text-sm text-slate-200">"Eigenwallet — version unknown"</div>
            <div class="mt-1 text-xs text-amber-400">{msg}</div>
        </div>
    }
}

#[component]
fn ValueSparkline(series: ChartSeries) -> impl IntoView {
    view! {
        <InteractiveLineChart points=series.points height=180 value_prefix="$"/>
    }
}

#[component]
fn OverviewBody(data: OverviewDto) -> impl IntoView {
    let btc = format_btc(data.btc_balance_sat);
    let xmr = format_xmr(&data.xmr_balance_atomic);
    let total_usd = data
        .total_usd
        .clone()
        .map(|v| format!("${}", trim_decimal(&v, 2)))
        .unwrap_or_else(|| "—".into());
    let peer_count = data
        .peer_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "—".into());
    let reg_text = data
        .registration
        .as_ref()
        .map(|r| format!("{}/{}", r.registered, r.total))
        .unwrap_or_else(|| "—".into());
    let reg_subtitle = data
        .registration
        .as_ref()
        .map(|r| {
            format!(
                "registered at {} of {} configured rendezvous nodes — peers discover us via these",
                r.registered, r.total
            )
        })
        .unwrap_or_else(|| "rendezvous nodes peers use to discover us".into());
    let onion_subtitle = if data.onion_addresses.is_empty() {
        "no hidden-service address yet — Tor still bootstrapping".to_string()
    } else {
        "Tor hidden-service is published — peers can reach us via .onion".to_string()
    };

    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
            <Tile title="BTC balance" subtitle="spendable BTC in the maker wallet">{btc}</Tile>
            <Tile title="XMR balance" subtitle="spendable XMR in the maker wallet">{xmr}</Tile>
            <Tile title="Total (USD)" subtitle="BTC + XMR valued at the latest CEX mid">{total_usd}</Tile>
            <Tile title="Active swaps" subtitle="swaps still in progress (not yet redeemed/refunded)">
                {data.active_swaps.to_string()}
            </Tile>
            <Tile title="Peers" subtitle="active libp2p connections">{peer_count}</Tile>
            <Tile title="Rendezvous" subtitle=reg_subtitle>{reg_text}</Tile>
            <Tile title="Spread" subtitle="our quoted price vs. CEX mid (positive = we charge a premium for XMR)">
                {data
                    .current_quote
                    .as_ref()
                    .and_then(|q| q.spread_pct.clone())
                    .map(|s| format!("+{}%", trim_decimal(&s, 2)))
                    .unwrap_or_else(|| "—".into())}
            </Tile>
            <Tile title="Onion" subtitle=onion_subtitle>
                {if data.onion_addresses.is_empty() {
                    "—".to_string()
                } else {
                    "reachable".to_string()
                }}
            </Tile>
        </div>
        <p class="text-xs text-slate-500">
            "Last updated " {data.as_of.to_rfc3339()}
        </p>
    }
}

#[component]
fn Loading() -> impl IntoView {
    view! { <div class="text-slate-400">"Loading…"</div> }
}

#[component]
fn ErrorBox(msg: String) -> impl IntoView {
    view! { <div class="tile border-rose-800 text-rose-300">{msg}</div> }
}

fn format_btc(sat: i64) -> String {
    let btc = sat as f64 / 100_000_000.0;
    format!("{:.5} BTC", btc)
}

fn format_xmr(atomic: &str) -> String {
    // 1 XMR = 1e12 atomic units (piconero).
    match atomic.parse::<u128>() {
        Ok(n) => {
            let whole = n / 1_000_000_000_000;
            let frac = n % 1_000_000_000_000;
            format!("{}.{:06} XMR", whole, frac / 1_000_000)
        }
        Err(_) => "— XMR".to_string(),
    }
}

fn trim_decimal(v: &str, places: usize) -> String {
    if let Some(dot) = v.find('.') {
        let end = (dot + 1 + places).min(v.len());
        v[..end].to_string()
    } else {
        v.to_string()
    }
}
