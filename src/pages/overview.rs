use leptos::prelude::*;

use crate::components::chart::InteractiveLineChart;
use crate::components::tile::Tile;
use crate::pages::health::get_health;
use crate::types::{
    HealthDto, HealthState, LifetimeRoiDto, MakerConfigUpdateResult, OverviewChartDto, OverviewDto,
    PauseStateDto, VersionInfoDto,
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

/// Composite endpoint for the Overview chart tile: USD value series for the
/// given period + capital-event markers in that window + the trading-only
/// P&L delta. One round-trip; see `OverviewChartDto`.
#[server(name = GetOverviewChart, prefix = "/api", endpoint = "overview/chart")]
pub async fn get_overview_chart(period: Option<String>) -> Result<OverviewChartDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::charts::overview_chart(&state, period.as_deref().unwrap_or("30d"))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn OverviewPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { get_overview().await });
    let version = Resource::new(|| (), |_| async move { get_version_info().await });
    let value = Resource::new(
        || (),
        |_| async move { get_overview_chart(Some("30d".into())).await },
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
            <div class="tile">
                <div class="tile-title">"Total value (USD, 30d)"</div>
                <Suspense fallback=move || view! { <div class="h-32 text-slate-400">"Loading…"</div> }>
                    {move || value.get().map(|res| match res {
                        Ok(d) => view! { <ValueSparkline data=d/> }.into_any(),
                        Err(e) => view! { <div class="text-rose-300">{e.to_string()}</div> }.into_any(),
                    })}
                </Suspense>
            </div>
            <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
                <HealthTile health=health/>
                <MakerTile pause=pause reload=pause_reload/>
                <VersionTile version=version/>
                <OverviewTile data=data kind=OverviewKind::TotalUsd/>

                <OverviewTile data=data kind=OverviewKind::Btc/>
                <OverviewTile data=data kind=OverviewKind::Xmr/>
                <OverviewTile data=data kind=OverviewKind::ActiveSwaps/>
                <OverviewTile data=data kind=OverviewKind::Swaps24h/>

                <OverviewTile data=data kind=OverviewKind::Spread/>
                <RoiTile roi=roi kind=RoiKind::Lifetime/>
                <RoiTile roi=roi kind=RoiKind::Holding/>
                <RoiTile roi=roi kind=RoiKind::Swaps/>
            </div>
            <p class="text-xs text-slate-500">
                <Suspense fallback=move || view! { <span>"Loading…"</span> }>
                    {move || data.get().and_then(|r| r.ok()).map(|d| format!("Last updated {}", d.as_of.to_rfc3339()))}
                </Suspense>
            </p>
        </div>
    }
}

#[component]
fn HealthTile(health: Resource<Result<HealthDto, ServerFnError>>) -> impl IntoView {
    view! {
        <div class="tile">
            <div class="flex items-baseline justify-between">
                <div class="tile-title">"Health"</div>
                <a href="/health" class="text-xs text-slate-500 hover:text-slate-300 underline">"details"</a>
            </div>
            <Suspense fallback=move || view! { <div class="tile-value text-slate-400">"…"</div> }>
                {move || health.get().map(|res| match res {
                    Ok(h) => render_health_body(h).into_any(),
                    Err(_) => view! {
                        <>
                            <div class="tile-value text-slate-400">"—"</div>
                            <div class="mt-1 text-xs text-amber-400">"check unavailable"</div>
                        </>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

fn render_health_body(data: HealthDto) -> impl IntoView {
    let subs: Vec<(&'static str, &str, HealthState)> = vec![
        ("asb", data.asb.headline.as_str(), data.asb.state),
        (
            "bitcoind",
            data.bitcoind.headline.as_str(),
            data.bitcoind.state,
        ),
        (
            "monerod",
            data.monerod.headline.as_str(),
            data.monerod.state,
        ),
        (
            "electrs",
            data.electrs.headline.as_str(),
            data.electrs.state,
        ),
        ("tor", data.tor.headline.as_str(), data.tor.state),
        ("peers", data.peers.headline.as_str(), data.peers.state),
        (
            "rendezvous",
            data.rendezvous.headline.as_str(),
            data.rendezvous.state,
        ),
        (
            "admin-db",
            data.admin_db.headline.as_str(),
            data.admin_db.state,
        ),
    ];
    let worst = subs
        .iter()
        .map(|(_, _, s)| *s)
        .max_by_key(|s| match s {
            HealthState::Ok => 0,
            HealthState::Degraded => 1,
            HealthState::Unknown => 2,
            HealthState::Down => 3,
        })
        .unwrap_or(HealthState::Unknown);
    let (color, word) = match worst {
        HealthState::Ok => ("text-emerald-300", "Healthy"),
        HealthState::Degraded => ("text-amber-300", "Degraded"),
        HealthState::Down => ("text-rose-300", "Down"),
        HealthState::Unknown => ("text-slate-300", "Unknown"),
    };
    let subtitle = subs
        .iter()
        .filter(|(_, _, s)| !matches!(s, HealthState::Ok))
        .map(|(n, h, _)| format!("{n}: {h}"))
        .collect::<Vec<_>>()
        .join(" · ");
    let subtitle = if subtitle.is_empty() {
        "all systems ok".to_string()
    } else {
        subtitle
    };
    view! {
        <div class=format!("tile-value {color}")>{word}</div>
        <div class="mt-1 text-xs text-slate-500">{subtitle}</div>
    }
}

#[component]
fn MakerTile(
    pause: Resource<Result<PauseStateDto, ServerFnError>>,
    reload: RwSignal<i32>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);

    view! {
        <div class="tile">
            <div class="tile-title">"Maker"</div>
            <Suspense fallback=move || view! { <div class="tile-value text-slate-400">"…"</div> }>
                {move || pause.get().map(|res| match res {
                    Ok(s) => {
                        let (word, color, button_label, button_class, paused) = if s.is_paused {
                            (
                                "PAUSED",
                                "text-rose-300",
                                "Resume",
                                "px-2 py-1 rounded bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-xs font-medium text-white",
                                true,
                            )
                        } else {
                            (
                                "LIVE",
                                "text-emerald-300",
                                "Pause",
                                "px-2 py-1 rounded bg-rose-700 hover:bg-rose-600 disabled:opacity-50 text-xs font-medium text-white",
                                false,
                            )
                        };
                        let on_click = move |_| {
                            if paused {
                                busy.set(true);
                                err.set(None);
                                leptos::task::spawn_local(async move {
                                    match resume_maker().await {
                                        Ok(_) => reload.update(|n| *n += 1),
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            } else {
                                if !web_sys::window()
                                    .and_then(|w| w.confirm_with_message(
                                        "Pause the maker?\n\nasb will quote off-market and stop accepting new swaps in ~30-60 s. In-flight swaps continue to settle.",
                                    ).ok())
                                    .unwrap_or(false)
                                {
                                    return;
                                }
                                busy.set(true);
                                err.set(None);
                                leptos::task::spawn_local(async move {
                                    match pause_maker().await {
                                        Ok(_) => reload.update(|n| *n += 1),
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            }
                        };
                        view! {
                            <div class=format!("tile-value {color}")>{word}</div>
                            <div class="mt-1 flex items-center gap-2">
                                <button class=button_class on:click=on_click prop:disabled=move || busy.get()>
                                    {move || if busy.get() { "…" } else { button_label }}
                                </button>
                                {move || err.get().map(|e| view! { <span class="text-xs text-rose-300">{e}</span> })}
                            </div>
                        }.into_any()
                    }
                    Err(e) => view! {
                        <>
                            <div class="tile-value text-slate-400">"—"</div>
                            <div class="mt-1 text-xs text-amber-400">{e.to_string()}</div>
                        </>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn VersionTile(version: Resource<Result<VersionInfoDto, ServerFnError>>) -> impl IntoView {
    view! {
        <div class="tile">
            <div class="tile-title">"Eigenwallet"</div>
            <Suspense fallback=move || view! { <div class="tile-value text-slate-400">"…"</div> }>
                {move || version.get().map(|res| match res {
                    Ok(info) => render_version_body(info).into_any(),
                    Err(_) => view! {
                        <>
                            <div class="tile-value">"—"</div>
                            <div class="mt-1 text-xs text-amber-400">"couldn't check for updates"</div>
                        </>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

fn render_version_body(info: VersionInfoDto) -> impl IntoView {
    let big = info
        .current
        .as_deref()
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "—".to_string());
    let subtitle = if info.fetch_error.is_some() {
        view! {
            <div class="mt-1 text-xs text-amber-400">"couldn't check for updates"</div>
        }
        .into_any()
    } else if info.has_update {
        let url = info
            .releases_url
            .clone()
            .unwrap_or_else(|| "https://github.com/eigenwallet/core/tags".into());
        let label = format!("↑ v{} — view changelog", info.latest.unwrap_or_default());
        view! {
            <div class="mt-1 text-xs">
                <a class="text-emerald-300 hover:text-emerald-200 underline"
                    href=url target="_blank" rel="noopener noreferrer">
                    {label}
                </a>
            </div>
        }
        .into_any()
    } else if info.current.is_some() {
        view! { <div class="mt-1 text-xs text-slate-500">"up to date"</div> }.into_any()
    } else {
        view! { <div class="mt-1 text-xs text-slate-500">"version unknown"</div> }.into_any()
    };
    view! {
        <div class="tile-value">{big}</div>
        {subtitle}
    }
}

#[derive(Clone, Copy)]
enum OverviewKind {
    Btc,
    Xmr,
    TotalUsd,
    ActiveSwaps,
    Swaps24h,
    Spread,
}

#[component]
fn OverviewTile(
    data: Resource<Result<OverviewDto, ServerFnError>>,
    kind: OverviewKind,
) -> impl IntoView {
    let (title, subtitle) = match kind {
        OverviewKind::Btc => ("BTC balance", "spendable BTC in the maker wallet"),
        OverviewKind::Xmr => ("XMR balance", "spendable XMR in the maker wallet"),
        OverviewKind::TotalUsd => ("Total (USD)", "BTC + XMR valued at the latest CEX mid"),
        OverviewKind::ActiveSwaps => (
            "Active swaps",
            "swaps still in progress (not yet redeemed/refunded)",
        ),
        OverviewKind::Swaps24h => (
            "Swaps (24h)",
            "completed swaps in the last 24 hours (redeem/refund/punish)",
        ),
        OverviewKind::Spread => (
            "Spread",
            "our quoted price vs. CEX mid (positive = we charge a premium for XMR)",
        ),
    };
    view! {
        <Tile title=title subtitle=subtitle>
            <Suspense fallback=move || view! { <span class="text-slate-400">"…"</span> }>
                {move || data.get().map(|res| match res {
                    Ok(d) => format_overview_value(&d, kind),
                    Err(_) => "—".to_string(),
                })}
            </Suspense>
        </Tile>
    }
}

fn format_overview_value(d: &OverviewDto, kind: OverviewKind) -> String {
    match kind {
        OverviewKind::Btc => format_btc(d.btc_balance_sat),
        OverviewKind::Xmr => format_xmr(&d.xmr_balance_atomic),
        OverviewKind::TotalUsd => d
            .total_usd
            .as_deref()
            .map(|v| format!("${}", trim_decimal(v, 2)))
            .unwrap_or_else(|| "—".into()),
        OverviewKind::ActiveSwaps => d.active_swaps.to_string(),
        OverviewKind::Swaps24h => d.swaps_24h.to_string(),
        OverviewKind::Spread => d
            .current_quote
            .as_ref()
            .and_then(|q| q.spread_pct.clone())
            .map(|s| format!("+{}%", trim_decimal(&s, 2)))
            .unwrap_or_else(|| "—".into()),
    }
}

#[derive(Clone, Copy)]
enum RoiKind {
    Lifetime,
    Holding,
    Swaps,
}

#[component]
fn RoiTile(roi: Resource<Result<LifetimeRoiDto, ServerFnError>>, kind: RoiKind) -> impl IntoView {
    let title = match kind {
        RoiKind::Lifetime => "Lifetime ROI",
        RoiKind::Holding => "P&L from holding",
        RoiKind::Swaps => "P&L from swaps",
    };
    let subtitle_static = match kind {
        RoiKind::Lifetime => None,
        RoiKind::Holding => Some("price moves on held crypto"),
        RoiKind::Swaps => Some("spread captured by swaps"),
    };

    view! {
        <div class="tile">
            <div class="tile-title">{title}</div>
            <Suspense fallback=move || view! { <div class="tile-value text-slate-400">"…"</div> }>
                {move || roi.get().map(|res| match res {
                    Ok(r) => render_roi_body(r, kind).into_any(),
                    Err(_) => view! {
                        <>
                            <div class="tile-value text-slate-400">"—"</div>
                        </>
                    }.into_any(),
                })}
            </Suspense>
            {subtitle_static.map(|s| view! { <div class="mt-1 text-xs text-slate-500">{s}</div> })}
        </div>
    }
}

fn render_roi_body(data: LifetimeRoiDto, kind: RoiKind) -> impl IntoView {
    let (value, secondary, color) = match kind {
        RoiKind::Lifetime => {
            let pct = data
                .roi_pct
                .as_deref()
                .map(|p| {
                    if p.trim().starts_with('-') {
                        p.to_string()
                    } else {
                        format!("+{p}")
                    }
                })
                .map(|s| format!("{s}%"))
                .unwrap_or_else(|| "—".to_string());
            let detail = if data.event_count == 0 {
                "Add capital events on the ROI page".to_string()
            } else {
                format!(
                    "{} P&L · ${} deployed",
                    format_signed_usd(&data.pnl_usd),
                    data.capital_deployed_usd,
                )
            };
            (pct, detail, sign_color(&data.pnl_usd))
        }
        RoiKind::Holding => {
            let v = data.market_pnl_usd.as_deref().unwrap_or("0").to_string();
            let formatted = format_signed_usd(&v);
            (formatted, String::new(), sign_color(&v))
        }
        RoiKind::Swaps => {
            let v = data.trade_pnl_usd.as_deref().unwrap_or("0").to_string();
            let formatted = format_signed_usd(&v);
            (formatted, String::new(), sign_color(&v))
        }
    };
    view! {
        <div class=format!("tile-value {color}")>{value}</div>
        {(!secondary.is_empty()).then(|| view! {
            <div class="mt-1 text-xs text-slate-400">{secondary}</div>
        })}
    }
}

fn sign_color(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.starts_with('-') {
        "text-rose-300"
    } else if trimmed == "0" || trimmed == "0.00" || trimmed.is_empty() {
        "text-slate-200"
    } else {
        "text-emerald-300"
    }
}

fn format_signed_usd(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('-') {
        format!("−${rest}")
    } else if trimmed == "0" || trimmed == "0.00" || trimmed.is_empty() {
        format!("${trimmed}")
    } else {
        format!("+${trimmed}")
    }
}

#[component]
fn ValueSparkline(data: OverviewChartDto) -> impl IntoView {
    let OverviewChartDto {
        series,
        markers,
        trade_only_delta_usd,
    } = data;
    view! {
        <InteractiveLineChart
            points=series.points
            height=180
            value_prefix="$"
            markers=markers
            trade_only_delta_usd=trade_only_delta_usd
        />
    }
}

fn format_btc(sat: i64) -> String {
    let btc = sat as f64 / 100_000_000.0;
    format!("{:.5} BTC", btc)
}

fn format_xmr(atomic: &str) -> String {
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
