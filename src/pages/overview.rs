use leptos::prelude::*;

use crate::components::chart::InteractiveLineChart;
use crate::components::tile::Tile;
use crate::pages::charts::get_account_value;
use crate::pages::health::get_health;
use crate::types::{
    ChartSeries, HealthDto, HealthState, LifetimeRoiDto, MakerConfigUpdateResult, OverviewDto,
    PauseStateDto, SubsystemHealth, VersionInfoDto,
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

#[server(name = GetLifetimeRoi, prefix = "/api", endpoint = "roi/lifetime")]
pub async fn get_lifetime_roi() -> Result<LifetimeRoiDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::roi::lifetime(&state)
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
    let roi = Resource::new(|| (), |_| async move { get_lifetime_roi().await });
    let health = Resource::new(|| (), |_| async move { get_health().await });

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
            <Suspense fallback=move || view! { <div class="text-sm text-slate-500">"Checking health…"</div> }>
                {move || health.get().map(|res| match res {
                    Ok(h) => view! { <HealthBanner data=h/> }.into_any(),
                    Err(e) => view! {
                        <div class="tile border-amber-700 text-amber-300 text-sm">
                            {format!("Health check unavailable: {e}")}
                        </div>
                    }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <div class="text-sm text-slate-500">"Loading ROI…"</div> }>
                {move || roi.get().map(|res| match res {
                    Ok(r) => view! { <LifetimeRoiTile data=r/> }.into_any(),
                    Err(e) => view! {
                        <div class="tile border-amber-700 text-amber-300 text-sm">
                            {format!("Lifetime ROI unavailable: {e}")}
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
fn HealthBanner(data: HealthDto) -> impl IntoView {
    let subs: Vec<(&'static str, SubsystemHealth)> = vec![
        ("asb", data.asb),
        ("bitcoind", data.bitcoind),
        ("monerod", data.monerod),
        ("electrs", data.electrs),
        ("tor", data.tor),
        ("peers", data.peers),
        ("rendezvous", data.rendezvous),
        ("admin-db", data.admin_db),
    ];
    let worst = subs
        .iter()
        .map(|(_, s)| s.state)
        .max_by_key(|s| match s {
            HealthState::Ok => 0,
            HealthState::Degraded => 1,
            HealthState::Unknown => 2,
            HealthState::Down => 3,
        })
        .unwrap_or(HealthState::Unknown);
    let (border, dot, label) = match worst {
        HealthState::Ok => (
            "border-emerald-700",
            "bg-emerald-400",
            "All systems operational",
        ),
        HealthState::Degraded => ("border-amber-700", "bg-amber-400", "Degraded"),
        HealthState::Down => ("border-rose-700", "bg-rose-500", "Problem detected"),
        HealthState::Unknown => ("border-slate-600", "bg-slate-400", "Status unknown"),
    };
    let bad: Vec<(&'static str, SubsystemHealth)> = subs
        .into_iter()
        .filter(|(_, s)| !matches!(s.state, HealthState::Ok))
        .collect();

    view! {
        <div class=format!("tile {border}")>
            <div class="flex items-center gap-2">
                <span class=format!("inline-block w-2.5 h-2.5 rounded-full {dot}")></span>
                <div class="text-sm font-medium text-slate-200">{label}</div>
                <a href="/health" class="ml-auto text-xs text-slate-400 hover:text-slate-200 underline">"details"</a>
            </div>
            {(!bad.is_empty()).then(|| view! {
                <ul class="mt-2 space-y-1 text-xs">
                    {bad.into_iter().map(|(name, s)| {
                        let color = match s.state {
                            HealthState::Down => "text-rose-300",
                            HealthState::Degraded => "text-amber-300",
                            _ => "text-slate-300",
                        };
                        view! {
                            <li class=color>
                                <span class="font-medium">{name}</span>": "{s.headline}
                            </li>
                        }
                    }).collect_view()}
                </ul>
            })}
        </div>
    }
}

#[component]
fn LifetimeRoiTile(data: LifetimeRoiDto) -> impl IntoView {
    // Parse PnL sign for color. roi_pct mirrors the sign so we key off it.
    let pnl_neg = data.pnl_usd.trim_start().starts_with('-');
    let pnl_zero = data.pnl_usd == "0" || data.pnl_usd == "0.00";
    let big_class = if pnl_zero {
        "text-2xl font-semibold text-slate-200"
    } else if pnl_neg {
        "text-2xl font-semibold text-rose-300"
    } else {
        "text-2xl font-semibold text-emerald-300"
    };
    let headline = match &data.roi_pct {
        Some(p) if !pnl_neg && !pnl_zero => format!("+{p}%"),
        Some(p) => format!("{p}%"),
        None => "—".into(),
    };
    let pnl_label = if pnl_neg {
        format!("−${}", data.pnl_usd.trim_start_matches('-'))
    } else {
        format!("+${}", data.pnl_usd)
    };
    let since_str = data
        .since
        .map(|t| t.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let primary = if data.event_count == 0 {
        "No capital events recorded yet — add some on the ROI page to see lifetime returns."
            .to_string()
    } else {
        format!(
            "Capital deployed: ${}  ·  Current value: ${}  ·  P&L: {}  ·  Since {} ({} event{})",
            data.capital_deployed_usd,
            data.current_value_usd,
            pnl_label,
            since_str,
            data.event_count,
            if data.event_count == 1 { "" } else { "s" },
        )
    };
    let breakdown = match (&data.market_pnl_usd, &data.trade_pnl_usd) {
        (Some(m), Some(t)) => Some(format!(
            "of which {} from holding (price moves) and {} from swaps",
            format_signed_usd(m),
            format_signed_usd(t),
        )),
        _ => None,
    };

    view! {
        <div class="tile">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
                <div class="tile-title">"Lifetime ROI"</div>
                <div class=big_class>{headline}</div>
            </div>
            <div class="mt-1 text-xs text-slate-400">{primary}</div>
            {breakdown.map(|b| view! {
                <div class="mt-0.5 text-xs text-slate-500">{b}</div>
            })}
        </div>
    }
}

fn format_signed_usd(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('-') {
        format!("−${rest}")
    } else if trimmed == "0" || trimmed == "0.00" {
        format!("${trimmed}")
    } else {
        format!("+${trimmed}")
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
    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
            <Tile title="BTC balance" subtitle="spendable BTC in the maker wallet">{btc}</Tile>
            <Tile title="XMR balance" subtitle="spendable XMR in the maker wallet">{xmr}</Tile>
            <Tile title="Total (USD)" subtitle="BTC + XMR valued at the latest CEX mid">{total_usd}</Tile>
            <Tile title="Active swaps" subtitle="swaps still in progress (not yet redeemed/refunded)">
                {data.active_swaps.to_string()}
            </Tile>
            <Tile title="Swaps (24h)" subtitle="completed swaps in the last 24 hours (redeem/refund/punish)">
                {data.swaps_24h.to_string()}
            </Tile>
            <Tile title="Spread" subtitle="our quoted price vs. CEX mid (positive = we charge a premium for XMR)">
                {data
                    .current_quote
                    .as_ref()
                    .and_then(|q| q.spread_pct.clone())
                    .map(|s| format!("+{}%", trim_decimal(&s, 2)))
                    .unwrap_or_else(|| "—".into())}
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
