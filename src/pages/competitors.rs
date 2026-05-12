use leptos::prelude::*;

use crate::types::{CompetitorScanDto, MarketPositionDto};

#[server(name = GetLatestScan, prefix = "/api", endpoint = "competitors/latest")]
pub async fn get_latest_scan() -> Result<Option<CompetitorScanDto>, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::competitors::latest(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = TriggerScan, prefix = "/api", endpoint = "competitors/scan")]
pub async fn trigger_scan() -> Result<String, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::competitors::trigger(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetMarketPosition, prefix = "/api", endpoint = "market/position")]
pub async fn get_market_position() -> Result<MarketPositionDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::market::position(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn CompetitorsPage() -> impl IntoView {
    let scan = Resource::new(|| (), |_| async move { get_latest_scan().await });
    let position = Resource::new(|| (), |_| async move { get_market_position().await });

    let scan_now = move |_| {
        leptos::task::spawn_local(async move {
            let _ = trigger_scan().await;
        });
    };

    view! {
        <div class="space-y-6">
            <div class="flex items-center justify-between">
                <h1 class="text-2xl font-semibold">"Competitors"</h1>
                <button class="btn btn-secondary" on:click=scan_now>"Scan now"</button>
            </div>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || position.get().map(|res| match res {
                    Ok(p) => view! { <PositionTile p=p/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || scan.get().map(|res| match res {
                    Ok(Some(s)) => view! { <ScanTable scan=s/> }.into_any(),
                    Ok(None) => view! { <div class="tile text-slate-400">"No completed scan yet."</div> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn PositionTile(p: MarketPositionDto) -> impl IntoView {
    let rank = p
        .rank_by_price
        .map(|r| format!("#{} of {}", r, p.total_active))
        .unwrap_or_else(|| "—".into());
    view! {
        <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
            <div class="tile">
                <div class="tile-title">"Our spread vs CEX mid"</div>
                <div class="tile-value">
                    {p.our_spread_pct.clone().map(|s| format!("+{}%", s)).unwrap_or_else(|| "—".into())}
                </div>
            </div>
            <div class="tile">
                <div class="tile-title">"Rank"</div>
                <div class="tile-value">{rank}</div>
            </div>
            <div class="tile">
                <div class="tile-title">"Cheapest competitor"</div>
                <div class="tile-value">
                    {p.cheapest_competitor_spread_pct
                        .clone()
                        .map(|s| format!("+{}%", s))
                        .unwrap_or_else(|| "—".into())}
                </div>
            </div>
        </div>
    }
}

#[component]
fn ScanTable(scan: CompetitorScanDto) -> impl IntoView {
    let started = scan.started_at.format("%Y-%m-%d %H:%M:%S").to_string();
    view! {
        <div class="tile overflow-x-auto">
            <div class="text-xs text-slate-500 mb-2">
                {format!("Scan {} • {}", scan.trigger, started)}
            </div>
            <table class="w-full text-sm">
                <thead>
                    <tr class="text-left text-xs uppercase text-slate-500">
                        <th class="py-2 pr-4">"Peer"</th>
                        <th class="py-2 pr-4">"Price (BTC/XMR)"</th>
                        <th class="py-2 pr-4">"Min"</th>
                        <th class="py-2 pr-4">"Max"</th>
                        <th class="py-2 pr-4">"Spread vs CEX"</th>
                        <th class="py-2 pr-4">"Status"</th>
                    </tr>
                </thead>
                <tbody>
                    {scan.quotes.into_iter().map(|q| {
                        let peer_short = if q.peer_id.len() > 16 {
                            format!("{}…{}", &q.peer_id[..8], &q.peer_id[q.peer_id.len() - 6..])
                        } else {
                            q.peer_id.clone()
                        };
                        let cls = if q.reachable { "badge-ok" } else { "badge-err" };
                        let status_text = if q.reachable {
                            "ok".to_string()
                        } else {
                            q.reason_if_unreachable.clone().unwrap_or_else(|| "—".into())
                        };
                        let row_class = if q.is_us {
                            "border-t border-indigo-500/40 bg-indigo-500/10 is-us"
                        } else {
                            "border-t border-slate-800"
                        };
                        let label = if q.is_us {
                            view! {
                                <span class="ml-2 inline-flex items-center rounded bg-indigo-600 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-white">"you"</span>
                            }.into_any()
                        } else {
                            ().into_any()
                        };
                        let is_us_attr = if q.is_us { "true" } else { "false" };
                        view! {
                            <tr class=row_class attr:data-is-us=is_us_attr>
                                <td class="py-2 pr-4 font-mono text-xs">
                                    {peer_short} {label}
                                </td>
                                <td class="py-2 pr-4">{q.price_btc_per_xmr.unwrap_or_else(|| "—".into())}</td>
                                <td class="py-2 pr-4">{q.min_btc.unwrap_or_else(|| "—".into())}</td>
                                <td class="py-2 pr-4">{q.max_btc.unwrap_or_else(|| "—".into())}</td>
                                <td class="py-2 pr-4">{q.spread_vs_cex_pct.unwrap_or_else(|| "—".into())}</td>
                                <td class="py-2 pr-4"><span class=cls>{status_text}</span></td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
