use leptos::prelude::*;

use crate::components::tile::Tile;
use crate::types::OverviewDto;

#[server(name = GetOverview, prefix = "/api", endpoint = "overview")]
pub async fn get_overview() -> Result<OverviewDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::overview::fetch(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn OverviewPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { get_overview().await });

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Overview"</h1>
            <Suspense fallback=move || view! { <Loading/> }>
                {move || match data.get() {
                    None => view! { <Loading/> }.into_any(),
                    Some(Err(e)) => view! { <ErrorBox msg=e.to_string()/> }.into_any(),
                    Some(Ok(d)) => view! { <OverviewBody data=d/> }.into_any(),
                }}
            </Suspense>
        </div>
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

    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
            <Tile title="BTC balance">{btc}</Tile>
            <Tile title="XMR balance">{xmr}</Tile>
            <Tile title="Total (USD)">{total_usd}</Tile>
            <Tile title="Active swaps">{data.active_swaps.to_string()}</Tile>
            <Tile title="Peers">{peer_count}</Tile>
            <Tile title="Rendezvous">{reg_text}</Tile>
            <Tile title="Spread">
                {data
                    .current_quote
                    .as_ref()
                    .and_then(|q| q.spread_pct.clone())
                    .map(|s| format!("+{}%", trim_decimal(&s, 2)))
                    .unwrap_or_else(|| "—".into())}
            </Tile>
            <Tile title="Onion">
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
