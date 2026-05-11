use leptos::prelude::*;

use crate::types::{AttributionDto, ChartPoint, ChartSeries};

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

#[server(name = GetAttribution, prefix = "/api", endpoint = "charts/attribution")]
pub async fn get_attribution(period: Option<String>) -> Result<AttributionDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::charts::attribution(&state, period.as_deref().unwrap_or("7d"))
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
    let attribution = Resource::new(
        move || period.get(),
        |p| async move { get_attribution(Some(p)).await },
    );

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Charts"</h1>
            <div class="flex gap-3 text-sm">
                <PeriodPicker period=period/>
                <DenomPicker denom=denom/>
            </div>
            <div class="tile">
                <div class="tile-title">"P&L attribution — actual vs. \"no trades\" baseline (USD)"</div>
                <Suspense fallback=move || view! { <div class="h-32 text-slate-400">"Loading…"</div> }>
                    {move || attribution.get().map(|res| match res {
                        Ok(a) => view! { <AttributionView a=a/> }.into_any(),
                        Err(e) => view! { <div class="text-rose-300">{e.to_string()}</div> }.into_any(),
                    })}
                </Suspense>
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

#[component]
fn AttributionView(a: AttributionDto) -> impl IntoView {
    if a.sample_count < 2 {
        return view! {
            <div class="mt-2 text-sm text-slate-400">
                "Need at least two balance snapshots to attribute P&L. Snapshots are taken every 5 min — this view fills in as the system runs."
                <div class="mt-1 text-xs text-slate-500">
                    {format!("Current samples: {}", a.sample_count)}
                </div>
            </div>
        }
        .into_any();
    }
    let svg = two_line_svg(&a.actual, &a.no_trade_baseline, 800, 200);
    let start = fmt_usd(&a.start_value_usd);
    let end = fmt_usd(&a.end_value_usd);
    let market = fmt_usd_signed(&a.market_pnl_usd);
    let trade = fmt_usd_signed(&a.trade_pnl_usd);
    let capital = fmt_usd_signed(&a.capital_flow_usd);
    let total = signed_diff(&a.end_value_usd, &a.start_value_usd);

    view! {
        <div class="mt-2 space-y-3">
            <div inner_html=svg></div>
            <div class="flex flex-wrap gap-2 text-xs">
                <Legend color="#818cf8" label="actual portfolio value"/>
                <Legend color="#94a3b8" label="if no swaps had happened (price + capital flow only)"/>
            </div>
            <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-2 text-xs">
                <StatCard label="Start" value=start/>
                <StatCard label="Now" value=end/>
                <StatCard label="Δ market" value=market hint="price moves on holdings"/>
                <StatCard label="Δ trades" value=trade hint="swap spread captured"/>
                <StatCard label="Δ capital" value=capital hint="deposits − withdrawals"/>
            </div>
            <div class="text-xs text-slate-500">
                {format!(
                    "Total change over {}: {}  ({} samples, sample every ~5 min)",
                    a.period, total, a.sample_count
                )}
            </div>
        </div>
    }
    .into_any()
}

#[component]
fn Legend(color: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <div class="flex items-center gap-1 text-slate-400">
            <span style=format!("display:inline-block;width:14px;height:2px;background:{};", color)></span>
            {label}
        </div>
    }
}

#[component]
fn StatCard(
    label: &'static str,
    value: String,
    #[prop(optional, into)] hint: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="rounded-md border border-slate-800 bg-slate-900/40 px-2 py-1.5">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">{label}</div>
            <div class="font-mono">{value}</div>
            {hint.map(|h| view! { <div class="text-[10px] text-slate-500 mt-0.5">{h}</div> })}
        </div>
    }
}

/// Format a non-negative f64 with comma thousands separators and 2 decimals.
fn fmt_thousands(v: f64) -> String {
    let s = format!("{:.2}", v);
    let (int_part, dec) = s.split_once('.').unwrap_or((&s, "00"));
    let mut grouped = String::new();
    for (i, ch) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let int_part: String = grouped.chars().rev().collect();
    format!("{int_part}.{dec}")
}

fn fmt_usd(s: &str) -> String {
    s.parse::<f64>()
        .map(|v| format!("${}", fmt_thousands(v)))
        .unwrap_or_else(|_| s.into())
}

fn fmt_usd_signed(s: &str) -> String {
    match s.parse::<f64>() {
        Ok(v) if v >= 0.0 => format!("+${}", fmt_thousands(v)),
        Ok(v) => format!("-${}", fmt_thousands(-v)),
        Err(_) => s.into(),
    }
}

fn signed_diff(end: &str, start: &str) -> String {
    let e = end.parse::<f64>().unwrap_or(0.0);
    let s = start.parse::<f64>().unwrap_or(0.0);
    fmt_usd_signed(&(e - s).to_string())
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

/// Two-line plot: actual (solid indigo) over no-trade-baseline (dashed slate).
/// Both series share the same time axis but each is scaled to the joint y-range.
fn two_line_svg(a: &[ChartPoint], b: &[ChartPoint], w: i32, h: i32) -> String {
    if a.is_empty() {
        return format!(
            "<svg viewBox='0 0 {w} {h}' class='w-full'><text x='10' y='20' fill='#64748b'>no data</text></svg>"
        );
    }
    let xy = |p: &ChartPoint| (p.t.timestamp() as f64, p.v.parse::<f64>().unwrap_or(0.0));
    let all_xy: Vec<(f64, f64)> = a.iter().chain(b.iter()).map(xy).collect();
    let xmin = all_xy.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let xmax = all_xy
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let ymin = all_xy.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let ymax = all_xy
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);
    let xspan = (xmax - xmin).max(1.0);
    let yspan = (ymax - ymin).max(1e-9);
    let pad_l = 56.0;
    let pad = 10.0;
    let to_x = |v: f64| pad_l + (v - xmin) / xspan * (w as f64 - pad_l - pad);
    let to_y = |v: f64| (h as f64 - pad) - (v - ymin) / yspan * (h as f64 - 2.0 * pad);
    let path = |pts: &[ChartPoint]| -> String {
        let mut d = String::new();
        for (i, p) in pts.iter().enumerate() {
            let (x, y) = xy(p);
            d.push_str(if i == 0 { "M " } else { " L " });
            d.push_str(&format!("{:.2} {:.2}", to_x(x), to_y(y)));
        }
        d
    };
    let actual_d = path(a);
    let baseline_d = path(b);

    // Y axis labels: min, mid, max
    let labels = [
        (ymax, to_y(ymax)),
        ((ymin + ymax) / 2.0, to_y((ymin + ymax) / 2.0)),
        (ymin, to_y(ymin)),
    ];
    let mut axis = String::new();
    for (val, yp) in &labels {
        let label = fmt_thousands(*val);
        // Truncate decimals from "12,345.67" -> "12,345" for compactness.
        let label_int = label.split_once('.').map(|(a, _)| a).unwrap_or(&label);
        axis.push_str(&format!(
            "<line x1='{pad_l}' y1='{yp:.1}' x2='{xmax_p}' y2='{yp:.1}' stroke='#1e293b' stroke-width='0.5'/>\
             <text x='{tx:.1}' y='{ty:.1}' fill='#64748b' font-size='10' text-anchor='end'>${label_int}</text>",
            xmax_p = w as f64 - pad,
            tx = pad_l - 4.0,
            ty = yp + 3.0,
        ));
    }

    format!(
        "<svg viewBox='0 0 {w} {h}' class='w-full'>\
           {axis}\
           <path d='{baseline_d}' fill='none' stroke='#94a3b8' stroke-width='1.3' stroke-dasharray='4 3'/>\
           <path d='{actual_d}' fill='none' stroke='#818cf8' stroke-width='1.6'/>\
         </svg>"
    )
}
