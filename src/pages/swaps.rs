use leptos::prelude::*;

use crate::types::{SwapListDto, SwapRow};

#[server(name = GetSwaps, prefix = "/api", endpoint = "swaps")]
pub async fn get_swaps(
    state_filter: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<SwapListDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::swaps::list(&state, state_filter.as_deref(), limit, offset)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn SwapsPage() -> impl IntoView {
    let filter = RwSignal::new(Option::<String>::None);
    let data = Resource::new(
        move || filter.get(),
        |f| async move { get_swaps(f, Some(50), Some(0)).await },
    );

    view! {
        <div class="space-y-4">
            <h1 class="text-2xl font-semibold">"Swaps"</h1>
            <div class="flex gap-2">
                <FilterButton current=filter label="all" value=None/>
                <FilterButton current=filter label="active" value=Some("active")/>
                <FilterButton current=filter label="completed" value=Some("completed")/>
                <FilterButton current=filter label="refunded" value=Some("refunded")/>
                <FilterButton current=filter label="punished" value=Some("punished")/>
            </div>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || match data.get() {
                    None => view! { <div class="text-slate-400">"Loading…"</div> }.into_any(),
                    Some(Err(e)) => {
                        view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any()
                    }
                    Some(Ok(d)) => view! { <SwapsTable rows=d.rows total=d.total/> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn FilterButton(
    current: RwSignal<Option<String>>,
    label: &'static str,
    value: Option<&'static str>,
) -> impl IntoView {
    let v = value.map(|s| s.to_string());
    let active = {
        let v = v.clone();
        Memo::new(move |_| current.get() == v)
    };
    view! {
        <button
            class=move || {
                if active.get() { "btn" } else { "btn btn-secondary" }
            }
            on:click=move |_| current.set(v.clone())
        >
            {label}
        </button>
    }
}

#[component]
fn SwapsTable(rows: Vec<SwapRow>, total: i64) -> impl IntoView {
    view! {
        <div class="tile overflow-x-auto">
            <table class="w-full text-sm">
                <thead>
                    <tr class="text-left text-xs uppercase text-slate-500">
                        <th class="py-2 pr-4">"State"</th>
                        <th class="py-2 pr-4">"Peer"</th>
                        <th class="py-2 pr-4">"BTC"</th>
                        <th class="py-2 pr-4">"XMR"</th>
                        <th class="py-2 pr-4">"Started"</th>
                        <th class="py-2 pr-4">"Profit (USD)"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|r| view! { <SwapRowView r=r/> }).collect_view()}
                </tbody>
            </table>
            <div class="mt-3 text-xs text-slate-500">{format!("{} total", total)}</div>
        </div>
    }
}

#[component]
fn SwapRowView(r: SwapRow) -> impl IntoView {
    let cls = match r.state.as_str() {
        "completed" => "badge-ok",
        "refunded" | "active" | "in-progress" => "badge-warn",
        "punished" => "badge-err",
        _ => "badge-warn",
    };
    let peer_short = if r.peer_id.len() > 16 {
        format!("{}…{}", &r.peer_id[..8], &r.peer_id[r.peer_id.len() - 6..])
    } else {
        r.peer_id.clone()
    };
    let btc = format!("{:.5}", r.btc_sat as f64 / 100_000_000.0);
    let xmr_atomic: u128 = r.xmr_atomic.parse().unwrap_or(0);
    let xmr = format!("{:.6}", xmr_atomic as f64 / 1e12);
    view! {
        <tr class="border-t border-slate-800">
            <td class="py-2 pr-4"><span class=cls>{r.state.clone()}</span></td>
            <td class="py-2 pr-4 font-mono text-xs">{peer_short}</td>
            <td class="py-2 pr-4">{btc}</td>
            <td class="py-2 pr-4">{xmr}</td>
            <td class="py-2 pr-4">{r.started_at.format("%Y-%m-%d %H:%M").to_string()}</td>
            <td class="py-2 pr-4">{r.profit_usd.unwrap_or_else(|| "—".into())}</td>
        </tr>
    }
}
