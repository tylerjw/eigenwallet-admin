use leptos::prelude::*;

use crate::types::{ChartPoint, ChartSeries};

#[server(name = GetAccountValue, prefix = "/api", endpoint = "charts/account-value")]
pub async fn get_account_value(
    period: Option<String>,
    denom: Option<String>,
) -> Result<ChartSeries, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::charts::account_value(
        &state,
        period.as_deref().unwrap_or("7d"),
        denom.as_deref().unwrap_or("usd"),
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = GetSwapCount, prefix = "/api", endpoint = "charts/swap-count")]
pub async fn get_swap_count(period: Option<String>) -> Result<ChartSeries, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::charts::swap_count(&state, period.as_deref().unwrap_or("30d"))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn ChartsPage() -> impl IntoView {
    let period = RwSignal::new("7d".to_string());
    let denom = RwSignal::new("usd".to_string());
    let value = Resource::new(
        move || (period.get(), denom.get()),
        |(p, d)| async move { get_account_value(Some(p), Some(d)).await },
    );
    let counts = Resource::new(
        move || period.get(),
        |p| async move { get_swap_count(Some(p)).await },
    );

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Charts"</h1>
            <div class="flex gap-3 text-sm">
                <PeriodPicker period=period/>
                <DenomPicker denom=denom/>
            </div>
            <div class="tile">
                <div class="tile-title">"Account value (" {move || denom.get()} ")"</div>
                <Suspense fallback=move || view! { <div class="h-32 text-slate-400">"Loading…"</div> }>
                    {move || value.get().map(|res| match res {
                        Ok(s) => view! { <SeriesView series=s/> }.into_any(),
                        Err(e) => view! { <div class="text-rose-300">{e.to_string()}</div> }.into_any(),
                    })}
                </Suspense>
            </div>
            <div class="tile">
                <div class="tile-title">"Swaps per day"</div>
                <Suspense fallback=move || view! { <div class="h-32 text-slate-400">"Loading…"</div> }>
                    {move || counts.get().map(|res| match res {
                        Ok(s) => view! { <SeriesView series=s/> }.into_any(),
                        Err(e) => view! { <div class="text-rose-300">{e.to_string()}</div> }.into_any(),
                    })}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn PeriodPicker(period: RwSignal<String>) -> impl IntoView {
    let opts = ["24h", "7d", "30d", "90d", "all"];
    view! {
        <div class="flex gap-1">
            {opts.iter().map(|o| {
                let o = o.to_string();
                let active = {
                    let o = o.clone();
                    Memo::new(move |_| period.get() == o)
                };
                let click_v = o.clone();
                view! {
                    <button
                        class=move || if active.get() { "btn" } else { "btn btn-secondary" }
                        on:click=move |_| period.set(click_v.clone())
                    >
                        {o.clone()}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn DenomPicker(denom: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="flex gap-1">
            <button
                class=move || if denom.get() == "usd" { "btn" } else { "btn btn-secondary" }
                on:click=move |_| denom.set("usd".to_string())
            >"USD"</button>
            <button
                class=move || if denom.get() == "btc" { "btn" } else { "btn btn-secondary" }
                on:click=move |_| denom.set("btc".to_string())
            >"BTC"</button>
        </div>
    }
}

#[component]
fn SeriesView(series: ChartSeries) -> impl IntoView {
    let svg = sparkline_svg(&series.points, 800, 160);
    let count = series.points.len();
    let last = series
        .points
        .last()
        .map(|p| p.v.clone())
        .unwrap_or_else(|| "—".into());
    view! {
        <div class="mt-2">
            <div inner_html=svg></div>
            <div class="mt-2 text-xs text-slate-500">
                {count} " samples • latest " {last}
            </div>
        </div>
    }
}

fn sparkline_svg(points: &[ChartPoint], w: i32, h: i32) -> String {
    if points.is_empty() {
        return format!(
            "<svg viewBox='0 0 {w} {h}' class='w-full'><text x='10' y='20' fill='#64748b'>no data</text></svg>"
        );
    }
    let xs: Vec<f64> = points.iter().map(|p| p.t.timestamp() as f64).collect();
    let ys: Vec<f64> = points
        .iter()
        .map(|p| p.v.parse::<f64>().unwrap_or(0.0))
        .collect();
    let xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let xmax = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let xspan = (xmax - xmin).max(1.0);
    let yspan = (ymax - ymin).max(1e-9);
    let pad = 8.0;
    let to_x = |v: f64| pad + (v - xmin) / xspan * (w as f64 - 2.0 * pad);
    let to_y = |v: f64| (h as f64 - pad) - (v - ymin) / yspan * (h as f64 - 2.0 * pad);
    let mut d = String::new();
    for (i, (x, y)) in xs.iter().zip(ys.iter()).enumerate() {
        d.push_str(if i == 0 { "M " } else { " L " });
        d.push_str(&format!("{:.2} {:.2}", to_x(*x), to_y(*y)));
    }
    format!(
        "<svg viewBox='0 0 {w} {h}' class='w-full'>\
           <path d='{d}' fill='none' stroke='#818cf8' stroke-width='1.5'/>\
         </svg>"
    )
}
