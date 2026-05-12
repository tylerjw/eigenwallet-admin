//! Kraken/Robinhood-style interactive line chart. Hover anywhere along the
//! line to display a big readout of the value at that point + the date and
//! a colored delta from the period start. A vertical crosshair follows the
//! cursor and snaps to the nearest data point.
//!
//! Renders correctly during SSR (just no crosshair until hydration); on the
//! client the `on:mousemove` handler drives a `cursor_idx` signal that
//! the readout, the crosshair, and the focus dot all key off.

use std::sync::Arc;

use leptos::prelude::*;
use leptos::web_sys;
use wasm_bindgen::JsCast;

use crate::types::{CapitalEventMarker, ChartPoint};

#[derive(Clone, Copy)]
pub struct ChartTheme {
    pub line: &'static str,
    pub dot: &'static str,
    pub guide: &'static str,
    pub axis: &'static str,
    pub up: &'static str,
    pub down: &'static str,
}

pub const USD_THEME: ChartTheme = ChartTheme {
    line: "#818cf8",
    dot: "#a5b4fc",
    guide: "#475569",
    axis: "#1e293b",
    up: "#34d399",
    down: "#fb7185",
};

/// Single-line interactive chart. `value_prefix` is shown before the readout
/// (e.g. "$" for USD); `height` is the SVG height in viewBox units.
///
/// `markers` (optional) decorates the plot with thin dotted vertical hairlines
/// at capital_event timestamps. When the cursor is near a marker, the readout
/// appends a short "+$X deposit (BTC)" / "−$X withdraw (XMR)" line.
///
/// `trade_only_delta_usd` (optional) adds a second small line under the main
/// delta readout: `"Trading only: +$XXX"`, colored green/red.
///
/// `pnl_cum_usd` + `capital_cum_usd` (both optional, must be the same length
/// as `points` if supplied): when present, the delta readout shows
/// `pnl_cum_usd[idx]` (market + trade P&L through the cursor) rather than
/// raw `value[idx] - value[0]`, and the % is normalized by
/// `value[0] + capital_cum_usd[idx]` so deposits don't tank the ratio.
#[component]
pub fn InteractiveLineChart(
    points: Vec<ChartPoint>,
    #[prop(default = USD_THEME)] theme: ChartTheme,
    #[prop(default = 200)] height: i32,
    #[prop(default = 1000)] width: i32,
    #[prop(default = "$")] value_prefix: &'static str,
    #[prop(optional, into)] markers: Option<Vec<CapitalEventMarker>>,
    #[prop(optional, into)] trade_only_delta_usd: Option<String>,
    #[prop(optional, into)] pnl_cum_usd: Option<Vec<String>>,
    #[prop(optional, into)] capital_cum_usd: Option<Vec<String>>,
) -> impl IntoView {
    if points.is_empty() {
        return view! {
            <div class="h-32 flex items-center justify-center text-slate-400 text-sm">
                "no data"
            </div>
        }
        .into_any();
    }
    if points.len() < 2 {
        let only = points[0].v.parse::<f64>().unwrap_or(0.0);
        return view! {
            <div class="mt-2 space-y-1">
                <div class="text-2xl font-mono text-slate-50">
                    {value_prefix} {fmt_thousands(only)}
                </div>
                <div class="text-xs text-slate-400">
                    "Only one sample so far — chart fills in every 5 min."
                </div>
            </div>
        }
        .into_any();
    }

    // Pre-compute everything in viewBox coordinates.
    let pad_l = 56.0;
    let pad_r = 16.0;
    let pad_t = 12.0;
    let pad_b = 22.0;
    let w = width as f64;
    let h = height as f64;
    let plot_w = w - pad_l - pad_r;
    let plot_h = h - pad_t - pad_b;

    let xs_t: Vec<f64> = points.iter().map(|p| p.t.timestamp() as f64).collect();
    let ys_v: Vec<f64> = points
        .iter()
        .map(|p| p.v.parse::<f64>().unwrap_or(0.0))
        .collect();
    let xmin = xs_t[0];
    let xmax = *xs_t.last().unwrap();
    let ymin = ys_v.iter().cloned().fold(f64::INFINITY, f64::min);
    let ymax = ys_v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let xspan = (xmax - xmin).max(1.0);
    let yspan = (ymax - ymin).max(1e-9);
    let to_x = |v: f64| pad_l + (v - xmin) / xspan * plot_w;
    let to_y = |v: f64| pad_t + plot_h - (v - ymin) / yspan * plot_h;

    let xs_px: Vec<f64> = xs_t.iter().map(|t| to_x(*t)).collect();
    let ys_px: Vec<f64> = ys_v.iter().map(|v| to_y(*v)).collect();
    let xs_px = Arc::new(xs_px);

    // Project markers into viewBox x coordinates. Drop any marker whose
    // timestamp falls outside the chart's x range (shouldn't happen with a
    // properly windowed server response, but defend against it).
    struct ProjectedMarker {
        x: f64,
        text: String,
    }
    let projected_markers: Vec<ProjectedMarker> = markers
        .as_ref()
        .map(|ms| {
            ms.iter()
                .filter_map(|m| {
                    let t = m.at.timestamp() as f64;
                    if t < xmin || t > xmax {
                        return None;
                    }
                    Some(ProjectedMarker {
                        x: to_x(t),
                        text: format_marker_text(m),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let projected_markers = Arc::new(projected_markers);

    // Trading-only delta readout (Overview only). Parse the signed decimal
    // string; if it doesn't parse, hide the line.
    let trade_only_parsed: Option<f64> = trade_only_delta_usd
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok());

    // Build the line path (smooth-ish via straight segments; could quadratic
    // smooth later but linear matches Kraken's default look).
    let mut path_d = String::new();
    for (i, (x, y)) in xs_px.iter().zip(ys_px.iter()).enumerate() {
        path_d.push_str(if i == 0 { "M " } else { " L " });
        path_d.push_str(&format!("{x:.2} {y:.2}"));
    }
    // A filled area gradient below the line.
    let mut area_d = path_d.clone();
    area_d.push_str(&format!(
        " L {x_end:.2} {y_bot:.2} L {x_start:.2} {y_bot:.2} Z",
        x_end = xs_px.last().unwrap(),
        x_start = xs_px.first().unwrap(),
        y_bot = pad_t + plot_h,
    ));

    // Y-axis labels: min / mid / max
    let y_labels: Vec<(f64, f64)> = [ymax, (ymin + ymax) / 2.0, ymin]
        .into_iter()
        .map(|v| (v, to_y(v)))
        .collect();

    // X-axis labels: first / middle / last
    let x_labels: Vec<(chrono::DateTime<chrono::Utc>, f64)> = [
        (points[0].t, xs_px[0]),
        (points[points.len() / 2].t, xs_px[points.len() / 2]),
        (points.last().unwrap().t, *xs_px.last().unwrap()),
    ]
    .into_iter()
    .collect();

    // Reactive cursor.
    let cursor: RwSignal<Option<usize>> = RwSignal::new(None);
    let last_idx = points.len() - 1;
    let points_arc = Arc::new(points);
    let ys_arc = Arc::new(ys_v);
    let pad_l_v = pad_l;
    let pad_t_v = pad_t;
    let plot_h_v = plot_h;
    let h_v = h;

    let on_move = {
        let xs = xs_px.clone();
        let w_screen_to_vb = move |target: &web_sys::Element, client_x: f64| -> f64 {
            let rect = target.get_bounding_client_rect();
            let mx = client_x - rect.x();
            (mx / rect.width()) * w
        };
        move |ev: leptos::ev::MouseEvent| {
            let Some(target) = ev.current_target() else {
                return;
            };
            let Ok(elem) = target.dyn_into::<web_sys::Element>() else {
                return;
            };
            let mx_vb = w_screen_to_vb(&elem, ev.client_x() as f64);
            // Find nearest xs index.
            let mut best = (0usize, f64::INFINITY);
            for (i, px) in xs.iter().enumerate() {
                let d = (*px - mx_vb).abs();
                if d < best.1 {
                    best = (i, d);
                }
            }
            cursor.set(Some(best.0));
        }
    };
    let on_leave = move |_| cursor.set(None);

    // Memos so we can call them from multiple reactive contexts without moving.
    let readout_value = Memo::new({
        let ys = ys_arc.clone();
        move |_| {
            let idx = cursor.get().unwrap_or(last_idx);
            let v = ys.get(idx).copied().unwrap_or(0.0);
            format!("{value_prefix}{}", fmt_thousands(v))
        }
    });
    let readout_when = Memo::new({
        let pts = points_arc.clone();
        move |_| {
            let idx = cursor.get().unwrap_or(last_idx);
            pts.get(idx)
                .map(|p| p.t.format("%b %-d, %Y %H:%M UTC").to_string())
                .unwrap_or_default()
        }
    });
    // If the caller passed pnl_cum + capital_cum aligned with the points
    // series, use them so the delta excludes new deposits/withdrawals.
    // Otherwise fall back to raw value-change-from-start.
    let n_points = points_arc.len();
    let pnl_arc: Option<Arc<Vec<f64>>> = pnl_cum_usd.filter(|v| v.len() == n_points).map(|v| {
        Arc::new(
            v.into_iter()
                .map(|s| s.parse::<f64>().unwrap_or(0.0))
                .collect(),
        )
    });
    let cap_arc: Option<Arc<Vec<f64>>> = capital_cum_usd.filter(|v| v.len() == n_points).map(|v| {
        Arc::new(
            v.into_iter()
                .map(|s| s.parse::<f64>().unwrap_or(0.0))
                .collect(),
        )
    });

    let readout_delta_text = Memo::new({
        let ys = ys_arc.clone();
        let pnl = pnl_arc.clone();
        let cap = cap_arc.clone();
        move |_| {
            let idx = cursor.get().unwrap_or(last_idx);
            let (delta, denom) = match (&pnl, &cap) {
                (Some(p), Some(c)) => {
                    let start_val = ys.first().copied().unwrap_or(0.0);
                    let d = p.get(idx).copied().unwrap_or(0.0);
                    let basis = start_val + c.get(idx).copied().unwrap_or(0.0);
                    (d, basis)
                }
                _ => {
                    let cur = ys.get(idx).copied().unwrap_or(0.0);
                    let start = ys.first().copied().unwrap_or(0.0);
                    (cur - start, start)
                }
            };
            let pct = if denom.abs() > 1e-9 {
                delta / denom * 100.0
            } else {
                0.0
            };
            let sign = if delta >= 0.0 { "+" } else { "−" };
            format!(
                "{sign}{value_prefix}{}  ({sign}{}%)",
                fmt_thousands(delta.abs()),
                fmt_thousands(pct.abs())
            )
        }
    });
    let readout_delta_color = Memo::new({
        let ys = ys_arc.clone();
        let pnl = pnl_arc.clone();
        move |_| {
            let idx = cursor.get().unwrap_or(last_idx);
            let delta = match &pnl {
                Some(p) => p.get(idx).copied().unwrap_or(0.0),
                None => {
                    let cur = ys.get(idx).copied().unwrap_or(0.0);
                    let start = ys.first().copied().unwrap_or(0.0);
                    cur - start
                }
            };
            if delta >= 0.0 { theme.up } else { theme.down }
        }
    });
    let xs_for_x = xs_px.clone();
    let cursor_x = Memo::new(move |_| cursor.get().and_then(|i| xs_for_x.get(i).copied()));
    let ys_for_y = Arc::new(ys_px);
    let cursor_y = Memo::new(move |_| cursor.get().and_then(|i| ys_for_y.get(i).copied()));

    // If the cursor is within ~5px of any marker, return its tooltip text.
    let marker_hit_text = Memo::new({
        let xs = xs_px.clone();
        let pm = projected_markers.clone();
        move |_| -> Option<String> {
            let idx = cursor.get()?;
            let cx = xs.get(idx).copied()?;
            pm.iter()
                .find(|m| (m.x - cx).abs() <= 5.0)
                .map(|m| m.text.clone())
        }
    });

    // Trade-only delta line — only shown if we got a non-empty, parseable value.
    let trade_only_line: Option<(String, &'static str)> = trade_only_parsed.map(|v| {
        let sign = if v >= 0.0 { "+" } else { "−" };
        let label = format!(
            "Trading only: {sign}{value_prefix}{}",
            fmt_thousands(v.abs())
        );
        let color = if v >= 0.0 { theme.up } else { theme.down };
        (label, color)
    });

    view! {
        <div class="mt-2">
            <div class="text-2xl md:text-3xl font-mono font-semibold text-slate-50 tabular-nums">
                {move || readout_value.get()}
            </div>
            <div class="text-xs mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5">
                <span class="text-slate-400">{move || readout_when.get()}</span>
                <span
                    class="font-mono tabular-nums"
                    style=move || format!("color: {};", readout_delta_color.get())
                >
                    {move || readout_delta_text.get()}
                </span>
                {move || marker_hit_text.get().map(|t| view! {
                    <span class="font-mono tabular-nums text-slate-400">{t}</span>
                })}
            </div>
            {trade_only_line.map(|(label, color)| view! {
                <div
                    class="text-xs mt-0.5 font-mono tabular-nums"
                    style=format!("color: {};", color)
                >
                    {label}
                </div>
            })}
            <svg
                viewBox=format!("0 0 {width} {height}")
                preserveAspectRatio="none"
                class="w-full mt-2 select-none touch-none"
                style=format!("height: {height}px; max-height: 220px;")
                on:mousemove=on_move
                on:mouseleave=on_leave
            >
                <defs>
                    <linearGradient id="ewa-chart-fill" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stop-color=theme.line stop-opacity="0.25"/>
                        <stop offset="100%" stop-color=theme.line stop-opacity="0"/>
                    </linearGradient>
                </defs>

                // y-axis grid + labels
                {y_labels.iter().map(|(val, yp)| {
                    let val = *val;
                    let yp = *yp;
                    view! {
                        <line
                            x1=pad_l_v y1=yp
                            x2=w - pad_r y2=yp
                            stroke=theme.axis stroke-width="0.5"
                        />
                        <text
                            x=pad_l_v - 4.0 y=yp + 3.0
                            fill="#64748b" font-size="10"
                            text-anchor="end"
                            class="font-mono tabular-nums"
                        >
                            {format!("{value_prefix}{}", fmt_axis(val))}
                        </text>
                    }
                }).collect_view()}

                // x-axis date labels
                {x_labels.iter().map(|(t, xp)| {
                    let xp = *xp;
                    let label = t.format("%b %-d").to_string();
                    let anchor = if xp < pad_l_v + 30.0 { "start" }
                        else if xp > w - pad_r - 30.0 { "end" }
                        else { "middle" };
                    view! {
                        <text
                            x=xp y=h_v - 6.0
                            fill="#64748b" font-size="10"
                            text-anchor=anchor
                        >
                            {label}
                        </text>
                    }
                }).collect_view()}

                // Filled area + line
                <path d=area_d fill="url(#ewa-chart-fill)"/>
                <path d=path_d fill="none" stroke=theme.line stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>

                // Capital-event markers: faint dotted vertical hairlines.
                {projected_markers.iter().map(|m| view! {
                    <line
                        x1=m.x y1=pad_t_v
                        x2=m.x y2=pad_t_v + plot_h_v
                        stroke="#64748b"
                        stroke-width="1"
                        stroke-dasharray="1 3"
                        stroke-opacity="0.55"
                    />
                }).collect_view()}

                // Crosshair (only when hovering)
                {move || cursor_x.get().map(|x| view! {
                    <line
                        x1=x y1=pad_t_v
                        x2=x y2=pad_t_v + plot_h_v
                        stroke=theme.guide
                        stroke-width="1"
                        stroke-dasharray="2 3"
                    />
                })}
                {move || match (cursor_x.get(), cursor_y.get()) {
                    (Some(x), Some(y)) => view! {
                        <circle cx=x cy=y r="3.5" fill=theme.dot stroke=theme.line stroke-width="2"/>
                    }.into_any(),
                    _ => ().into_any(),
                }}
            </svg>
        </div>
    }
    .into_any()
}

/// Format a float with comma thousand separators and 2 decimals.
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

/// Tooltip text for a capital-event marker, e.g.
/// `"+$2,000 deposit (BTC)"` or `"−$500 withdraw (XMR)"`.
/// USD value missing → drops the dollar amount, e.g. `"deposit (XMR)"`.
fn format_marker_text(m: &CapitalEventMarker) -> String {
    let usd_part = m.usd_value.as_deref().and_then(|s| s.parse::<f64>().ok());
    match (usd_part, m.direction.as_str()) {
        (Some(v), "withdraw") => format!("−${} withdraw ({})", fmt_thousands(v.abs()), m.asset),
        (Some(v), _) => format!("+${} deposit ({})", fmt_thousands(v.abs()), m.asset),
        (None, "withdraw") => format!("withdraw ({})", m.asset),
        (None, _) => format!("deposit ({})", m.asset),
    }
}

/// Compact y-axis label. Drops decimals above 1k.
fn fmt_axis(v: f64) -> String {
    if v.abs() >= 1000.0 {
        let s = fmt_thousands(v);
        s.split_once('.').map(|(a, _)| a.to_string()).unwrap_or(s)
    } else {
        fmt_thousands(v)
    }
}
