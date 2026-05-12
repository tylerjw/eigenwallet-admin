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

/// The set of filter tokens recognised by the server. Each variant maps to a
/// substring predicate against `swaps.state` (see `build_state_predicate` in
/// `src/server/api/swaps.rs`). We keep this as a plain enum on the client so
/// the filter signal carries a `Copy` value, sidesteps any `Option<String>`
/// clone gymnastics in event handlers, and gives the buttons a single source
/// of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapFilter {
    All,
    Active,
    Completed,
    Refunded,
    Punished,
}

impl SwapFilter {
    fn label(self) -> &'static str {
        match self {
            SwapFilter::All => "all",
            SwapFilter::Active => "active",
            SwapFilter::Completed => "completed",
            SwapFilter::Refunded => "refunded",
            SwapFilter::Punished => "punished",
        }
    }

    /// The token the server-side filter expects, or `None` for "all".
    fn token(self) -> Option<String> {
        match self {
            SwapFilter::All => None,
            other => Some(other.label().to_string()),
        }
    }
}

#[component]
pub fn SwapsPage() -> impl IntoView {
    let filter = RwSignal::new(SwapFilter::All);
    let data = Resource::new(
        move || filter.get(),
        // No limit/offset: render every matching row. The dataset is small
        // (a few thousand swaps over the maker's lifetime) so a single
        // full-page render is fine and there's no pager UI to drive paging.
        |f| async move { get_swaps(f.token(), None, None).await },
    );

    view! {
        <div class="space-y-4">
            <h1 class="text-2xl font-semibold">"Swaps"</h1>
            <div class="flex gap-2">
                <FilterButton current=filter value=SwapFilter::All/>
                <FilterButton current=filter value=SwapFilter::Active/>
                <FilterButton current=filter value=SwapFilter::Completed/>
                <FilterButton current=filter value=SwapFilter::Refunded/>
                <FilterButton current=filter value=SwapFilter::Punished/>
            </div>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || match data.get() {
                    None => view! { <div class="text-slate-400">"Loading…"</div> }.into_any(),
                    Some(Err(e)) => {
                        view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any()
                    }
                    Some(Ok(d)) => {
                        let f = filter.get();
                        view! { <SwapsTable rows=d.rows total=d.total filter=f/> }.into_any()
                    }
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn FilterButton(current: RwSignal<SwapFilter>, value: SwapFilter) -> impl IntoView {
    // `value` is `Copy`, so the on:click closure can simply read it. Earlier
    // versions cloned an `Option<String>` per click, which still worked but
    // made the handler harder to reason about during hydration debugging.
    view! {
        <button
            class=move || {
                if current.get() == value { "btn" } else { "btn btn-secondary" }
            }
            on:click=move |_| current.set(value)
        >
            {value.label()}
        </button>
    }
}

#[component]
fn SwapsTable(rows: Vec<SwapRow>, total: i64, filter: SwapFilter) -> impl IntoView {
    let shown = rows.len();
    // Sanity-check label: when a filter is active, show "N filtered" so the
    // operator can see that the filter actually narrowed the result. The
    // server's `total` is computed against the *same* predicate as the rows,
    // so `total == shown` whenever the result set fits in one page (which it
    // always does now that the server returns every match).
    let summary = match filter {
        SwapFilter::All => format!("{total} total"),
        other => format!("{total} {} (of all swaps)", other.label()),
    };
    let row_count_hint = if (shown as i64) < total {
        Some(format!(" — showing {shown}"))
    } else {
        None
    };
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
            <div class="mt-3 text-xs text-slate-500">
                {summary}{row_count_hint}
            </div>
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
