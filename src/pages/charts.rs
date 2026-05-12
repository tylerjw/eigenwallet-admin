use std::sync::Arc;

use leptos::prelude::*;

use crate::components::chart::InteractiveLineChart;
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

/// Period button row. Explicit buttons (no `iter().map().collect_view()`) so
/// hydration reliably attaches the `on:click` handlers. The previous
/// iterator-based form rendered fine but didn't always rewire its events
/// post-hydration, so the chart appeared frozen on the initial 7d window.
#[component]
fn PeriodPicker(period: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="flex gap-1">
            <PeriodButton period=period value="24h"/>
            <PeriodButton period=period value="7d"/>
            <PeriodButton period=period value="30d"/>
            <PeriodButton period=period value="90d"/>
            <PeriodButton period=period value="all"/>
        </div>
    }
}

#[component]
fn PeriodButton(period: RwSignal<String>, value: &'static str) -> impl IntoView {
    view! {
        <button
            class=move || if period.get() == value { "btn" } else { "btn btn-secondary" }
            on:click=move |_| period.set(value.to_string())
        >
            {value}
        </button>
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
    let prefix: &'static str = match series.denomination.as_str() {
        "usd" => "$",
        _ => "",
    };
    view! {
        <InteractiveLineChart points=series.points height=200 value_prefix=prefix/>
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

    // Hovered sample index drives the StatCard values. None = period totals.
    let hovered: RwSignal<Option<usize>> = RwSignal::new(None);

    // Period totals as fallback when not hovering.
    let total_market = a.market_pnl_usd.clone();
    let total_trade = a.trade_pnl_usd.clone();
    let total_capital = a.capital_flow_usd.clone();
    let start_str = fmt_usd(&a.start_value_usd);
    let total_str = signed_diff(&a.end_value_usd, &a.start_value_usd);
    let total_str_static = total_str.clone();

    let actual = Arc::new(a.actual.clone());
    let baseline = Arc::new(a.no_trade_baseline.clone());
    let market_cum = Arc::new(a.market_cum.clone());
    let trade_cum = Arc::new(a.trade_cum.clone());
    let capital_cum = Arc::new(a.capital_cum.clone());

    let period_label = a.period.clone();
    let sample_count = a.sample_count;
    let missing_usd = a.capital_events_missing_usd;
    let total_events = a.capital_events_total;

    // "End" / "Now" follows the hovered sample's actual value; falls back to
    // the period end. Same for market / trade / capital StatCards.
    let end_label = {
        let actual = actual.clone();
        Memo::new(move |_| {
            let idx = hovered
                .get()
                .unwrap_or_else(|| actual.len().saturating_sub(1));
            actual
                .get(idx)
                .map(|p| fmt_usd(&p.v))
                .unwrap_or_else(|| "—".into())
        })
    };
    let market_label = {
        let series = market_cum.clone();
        let fallback = total_market.clone();
        Memo::new(move |_| match hovered.get() {
            Some(idx) => series
                .get(idx)
                .map(|p| fmt_usd_signed(&p.v))
                .unwrap_or_else(|| fmt_usd_signed(&fallback)),
            None => fmt_usd_signed(&fallback),
        })
    };
    let trade_label = {
        let series = trade_cum.clone();
        let fallback = total_trade.clone();
        Memo::new(move |_| match hovered.get() {
            Some(idx) => series
                .get(idx)
                .map(|p| fmt_usd_signed(&p.v))
                .unwrap_or_else(|| fmt_usd_signed(&fallback)),
            None => fmt_usd_signed(&fallback),
        })
    };
    let capital_label = {
        let series = capital_cum.clone();
        let fallback = total_capital.clone();
        Memo::new(move |_| match hovered.get() {
            Some(idx) => series
                .get(idx)
                .map(|p| fmt_usd_signed(&p.v))
                .unwrap_or_else(|| fmt_usd_signed(&fallback)),
            None => fmt_usd_signed(&fallback),
        })
    };
    let total_label = {
        let actual = actual.clone();
        let start_val: f64 = a.start_value_usd.parse().unwrap_or(0.0);
        let fallback = total_str.clone();
        Memo::new(move |_| match hovered.get() {
            Some(idx) => actual
                .get(idx)
                .map(|p| {
                    let v: f64 = p.v.parse().unwrap_or(0.0);
                    fmt_usd_signed(&(v - start_val).to_string())
                })
                .unwrap_or_else(|| fallback.clone()),
            None => fallback.clone(),
        })
    };

    view! {
        <div class="mt-2 space-y-3">
            <AttributionSvg
                actual=actual.clone()
                baseline=baseline.clone()
                hovered=hovered
            />
            <div class="flex flex-wrap gap-2 text-xs">
                <Legend color="#818cf8" label="actual portfolio value"/>
                <Legend color="#94a3b8" label="if no swaps had happened (price + capital flow only)"/>
            </div>
            <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-2 text-xs">
                <StatCard label="Start" value=Signal::derive(move || start_str.clone())/>
                <StatCard label="Now" value=Signal::derive(move || end_label.get())/>
                <StatCard label="Δ market" value=Signal::derive(move || market_label.get()) hint="price moves on holdings"/>
                <StatCard label="Δ trades" value=Signal::derive(move || trade_label.get()) hint="swap spread captured"/>
                <StatCard label="Δ capital" value=Signal::derive(move || capital_label.get()) hint="deposits − withdrawals"/>
            </div>
            <div class="text-xs text-slate-500">
                {move || format!(
                    "Total change over {}: {}  ({} samples, sample every ~5 min)",
                    period_label.clone(), total_label.get(), sample_count
                )}
                <span class="ml-2 text-slate-600">
                    {format!("[period total: {total_str_static}]")}
                </span>
                {if missing_usd > 0 {
                    Some(view! {
                        <div class="mt-1 text-amber-400">
                            {format!(
                                "Note: {missing_usd} of {total_events} capital_events in this window had a NULL usd_value_at_event; \
                                 the server estimated USD from the nearest snapshot price. Trade-P&L confidence is reduced; \
                                 backfill historical USD prices on those rows for an exact number.",
                            )}
                        </div>
                    })
                } else { None }}
            </div>
        </div>
    }
    .into_any()
}

/// Two-line attribution SVG with reactive hover. Mirrors the readout/crosshair
/// pattern from `InteractiveLineChart` but renders two paths (actual + dashed
/// no-trade baseline). On hover, writes the nearest-x sample index into
/// `hovered`, which the parent's StatCards key off.
#[component]
fn AttributionSvg(
    actual: Arc<Vec<ChartPoint>>,
    baseline: Arc<Vec<ChartPoint>>,
    hovered: RwSignal<Option<usize>>,
) -> impl IntoView {
    let w: i32 = 800;
    let h: i32 = 200;
    let pad_l = 56.0f64;
    let pad_r = 10.0f64;
    let pad_t = 10.0f64;
    let pad_b = 10.0f64;
    let wf = w as f64;
    let hf = h as f64;

    if actual.is_empty() {
        return view! {
            <svg viewBox=format!("0 0 {w} {h}") class="w-full">
                <text x="10" y="20" fill="#64748b">"no data"</text>
            </svg>
        }
        .into_any();
    }

    let xy = |p: &ChartPoint| (p.t.timestamp() as f64, p.v.parse::<f64>().unwrap_or(0.0));
    let all_xy: Vec<(f64, f64)> = actual.iter().chain(baseline.iter()).map(xy).collect();
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
    let to_x = move |v: f64| pad_l + (v - xmin) / xspan * (wf - pad_l - pad_r);
    let to_y = move |v: f64| (hf - pad_b) - (v - ymin) / yspan * (hf - pad_t - pad_b);

    let xs_px: Vec<f64> = actual.iter().map(|p| to_x(xy(p).0)).collect();
    let xs_px = Arc::new(xs_px);

    let path_of = |pts: &[ChartPoint]| -> String {
        let mut d = String::new();
        for (i, p) in pts.iter().enumerate() {
            let (x, y) = xy(p);
            d.push_str(if i == 0 { "M " } else { " L " });
            d.push_str(&format!("{:.2} {:.2}", to_x(x), to_y(y)));
        }
        d
    };
    let actual_d = path_of(&actual);
    let baseline_d = path_of(&baseline);

    // Y axis labels: min, mid, max
    let labels = [
        (ymax, to_y(ymax)),
        ((ymin + ymax) / 2.0, to_y((ymin + ymax) / 2.0)),
        (ymin, to_y(ymin)),
    ];

    let xs_for_handler = xs_px.clone();
    let on_move = move |ev: leptos::ev::MouseEvent| {
        use wasm_bindgen::JsCast;
        let Some(target) = ev.current_target() else {
            return;
        };
        let Ok(elem) = target.dyn_into::<leptos::web_sys::Element>() else {
            return;
        };
        let rect = elem.get_bounding_client_rect();
        let mx = ev.client_x() as f64 - rect.x();
        let mx_vb = (mx / rect.width()) * wf;
        let mut best = (0usize, f64::INFINITY);
        for (i, px) in xs_for_handler.iter().enumerate() {
            let d = (*px - mx_vb).abs();
            if d < best.1 {
                best = (i, d);
            }
        }
        hovered.set(Some(best.0));
    };
    let on_leave = move |_| hovered.set(None);

    let xs_for_cursor = xs_px.clone();
    let actual_for_cursor = actual.clone();
    let cursor_xy = Memo::new(move |_| {
        let idx = hovered.get()?;
        let x = xs_for_cursor.get(idx).copied()?;
        let y = actual_for_cursor.get(idx).map(|p| to_y(xy(p).1))?;
        Some((x, y))
    });

    view! {
        <svg
            viewBox=format!("0 0 {w} {h}")
            preserveAspectRatio="none"
            class="w-full select-none touch-none"
            style=format!("height: {h}px;")
            on:mousemove=on_move
            on:mouseleave=on_leave
        >
            {labels.iter().map(|(val, yp)| {
                let val = *val;
                let yp = *yp;
                let label = fmt_thousands(val);
                let label_int = label.split_once('.').map(|(a, _)| a.to_string()).unwrap_or(label);
                view! {
                    <line
                        x1=pad_l y1=yp
                        x2=wf - pad_r y2=yp
                        stroke="#1e293b" stroke-width="0.5"
                    />
                    <text
                        x=pad_l - 4.0 y=yp + 3.0
                        fill="#64748b" font-size="10"
                        text-anchor="end"
                    >
                        {format!("${label_int}")}
                    </text>
                }
            }).collect_view()}
            <path d=baseline_d fill="none" stroke="#94a3b8" stroke-width="1.3" stroke-dasharray="4 3"/>
            <path d=actual_d fill="none" stroke="#818cf8" stroke-width="1.6"/>

            {move || cursor_xy.get().map(|(x, _y)| view! {
                <line
                    x1=x y1=pad_t
                    x2=x y2=hf - pad_b
                    stroke="#475569"
                    stroke-width="1"
                    stroke-dasharray="2 3"
                />
            })}
            {move || cursor_xy.get().map(|(x, y)| view! {
                <circle cx=x cy=y r="3.5" fill="#a5b4fc" stroke="#818cf8" stroke-width="2"/>
            })}
        </svg>
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
    #[prop(into)] value: Signal<String>,
    #[prop(optional, into)] hint: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="rounded-md border border-slate-800 bg-slate-900/40 px-2 py-1.5">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">{label}</div>
            <div class="font-mono">{move || value.get()}</div>
            {hint.map(|h| view! { <div class="text-[10px] text-slate-500 mt-0.5">{h}</div> })}
        </div>
    }
}

/// Format a non-negative f64 with comma thousands separators and 2 decimals.
fn fmt_thousands(v: f64) -> String {
    let s = format!("{:.2}", v);
    let (int_part, dec) = s.split_once('.').unwrap_or((&s, "00"));
    let neg = int_part.starts_with('-');
    let digits = int_part.trim_start_matches('-');
    let mut grouped = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let int_part: String = grouped.chars().rev().collect();
    if neg {
        format!("-{int_part}.{dec}")
    } else {
        format!("{int_part}.{dec}")
    }
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
