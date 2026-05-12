use leptos::prelude::*;

use crate::pages::overview::get_lifetime_roi;
use crate::types::{CapitalEventDto, CapitalEventInput, LifetimeRoiDto};

#[server(name = ListCapitalEvents, prefix = "/api", endpoint = "capital-events/list")]
pub async fn list_capital_events() -> Result<Vec<CapitalEventDto>, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::capital::list(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = AddCapitalEvent, prefix = "/api", endpoint = "capital-events/add")]
pub async fn add_capital_event(input: CapitalEventInput) -> Result<CapitalEventDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::capital::add(&state, input)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn RoiPage() -> impl IntoView {
    let roi = Resource::new(|| (), |_| async move { get_lifetime_roi().await });
    let events = Resource::new(|| (), |_| async move { list_capital_events().await });

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"ROI & capital"</h1>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || roi.get().map(|res| match res {
                    Ok(r) => view! { <RoiTile r=r/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
            <CapitalEventForm on_added=move || events.refetch()/>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || events.get().map(|res| match res {
                    Ok(list) => view! { <EventsTable rows=list/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn RoiTile(r: LifetimeRoiDto) -> impl IntoView {
    let pnl_neg = r.pnl_usd.trim_start().starts_with('-');
    let pnl_zero = r.pnl_usd == "0" || r.pnl_usd == "0.00";
    let big_class = if pnl_zero {
        "tile-value text-slate-200"
    } else if pnl_neg {
        "tile-value text-rose-300"
    } else {
        "tile-value text-emerald-300"
    };
    let headline = match &r.roi_pct {
        Some(p) if !pnl_neg && !pnl_zero => format!("+{p}%"),
        Some(p) => format!("{p}%"),
        None => "—".into(),
    };
    let since_str = r
        .since
        .map(|t| t.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let pnl_label = if pnl_neg {
        format!("−${}", r.pnl_usd.trim_start_matches('-'))
    } else {
        format!("+${}", r.pnl_usd)
    };
    let primary = format!(
        "${} deployed · ${} current · P&L {} · since {} ({} event{})",
        r.capital_deployed_usd,
        r.current_value_usd,
        pnl_label,
        since_str,
        r.event_count,
        if r.event_count == 1 { "" } else { "s" },
    );
    let breakdown = match (&r.market_pnl_usd, &r.trade_pnl_usd) {
        (Some(m), Some(t)) => Some(format!(
            "of which {} from holding (price moves) and {} from swaps",
            format_signed_usd(m),
            format_signed_usd(t),
        )),
        _ => None,
    };

    view! {
        <div class="tile">
            <div class="tile-title">"Lifetime ROI"</div>
            <div class=big_class>{headline}</div>
            <div class="text-xs text-slate-400 mt-1">{primary}</div>
            {breakdown.map(|b| view! { <div class="text-xs text-slate-500 mt-0.5">{b}</div> })}
        </div>
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
fn CapitalEventForm(on_added: impl Fn() + 'static + Clone + Send) -> impl IntoView {
    // Default "occurred_at" = now, formatted as datetime-local (no TZ suffix).
    let now_local = chrono::Utc::now().format("%Y-%m-%dT%H:%M").to_string();
    let occurred = RwSignal::new(now_local);
    let direction = RwSignal::new("deposit".to_string());
    let asset = RwSignal::new("BTC".to_string());
    let amount = RwSignal::new(String::new());
    let usd = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let status = RwSignal::new(Option::<String>::None);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        // Parse the datetime-local value `YYYY-MM-DDTHH:MM` as UTC.
        let occurred_at =
            match chrono::NaiveDateTime::parse_from_str(occurred.get().trim(), "%Y-%m-%dT%H:%M") {
                Ok(n) => n.and_utc(),
                Err(_) => {
                    status.set(Some("FAIL: invalid date".into()));
                    return;
                }
            };
        let input = CapitalEventInput {
            occurred_at,
            direction: direction.get(),
            asset: asset.get(),
            amount: amount.get(),
            usd_value_at_event: if usd.get().trim().is_empty() {
                None
            } else {
                Some(usd.get())
            },
            notes: if notes.get().trim().is_empty() {
                None
            } else {
                Some(notes.get())
            },
        };
        let on_added = on_added.clone();
        leptos::task::spawn_local(async move {
            match add_capital_event(input).await {
                Ok(_) => {
                    status.set(Some("Recorded.".into()));
                    amount.set(String::new());
                    usd.set(String::new());
                    notes.set(String::new());
                    on_added();
                }
                Err(e) => status.set(Some(format!("FAIL: {e}"))),
            }
        });
    };

    let amount_label = move || match asset.get().as_str() {
        "BTC" => "Amount (BTC)",
        "XMR" => "Amount (XMR)",
        _ => "Amount",
    };

    view! {
        <form class="tile space-y-3" on:submit=on_submit>
            <div class="tile-title">"Record capital event"</div>
            <p class="text-xs text-slate-400">
                "Record a deposit (BTC or XMR moved INTO the wallet from cold storage / an exchange) "
                "or a withdrawal (moved OUT to cold storage). The system can't auto-detect these — incoming "
                "BTC also arrives from swap counterparties, so only you know which transactions are real capital "
                "events vs. trading flow. Enter the date the funds actually moved on-chain."
            </p>
            <div class="grid grid-cols-2 md:grid-cols-6 gap-3">
                <label class="text-xs uppercase text-slate-400 md:col-span-2">
                    "When (UTC)"
                    <input
                        type="datetime-local"
                        class="input mt-1"
                        prop:value=move || occurred.get()
                        on:input=move |ev| occurred.set(event_target_value(&ev))
                    />
                </label>
                <label class="text-xs uppercase text-slate-400">
                    "Direction"
                    <select
                        class="input mt-1"
                        on:change=move |ev| direction.set(event_target_value(&ev))
                    >
                        <option value="deposit">"deposit"</option>
                        <option value="withdraw">"withdraw"</option>
                    </select>
                </label>
                <label class="text-xs uppercase text-slate-400">
                    "Asset"
                    <select
                        class="input mt-1"
                        on:change=move |ev| asset.set(event_target_value(&ev))
                    >
                        <option value="BTC">"BTC"</option>
                        <option value="XMR">"XMR"</option>
                    </select>
                </label>
                <label class="text-xs uppercase text-slate-400">
                    {amount_label}
                    <input
                        class="input mt-1"
                        placeholder="e.g. 1.5"
                        prop:value=move || amount.get()
                        on:input=move |ev| amount.set(event_target_value(&ev))
                    />
                </label>
                <label class="text-xs uppercase text-slate-400">
                    "USD value (optional)"
                    <input
                        class="input mt-1"
                        placeholder="auto for recent"
                        prop:value=move || usd.get()
                        on:input=move |ev| usd.set(event_target_value(&ev))
                    />
                </label>
            </div>
            <label class="text-xs uppercase text-slate-400 block">
                "Notes"
                <input
                    class="input mt-1"
                    prop:value=move || notes.get()
                    on:input=move |ev| notes.set(event_target_value(&ev))
                />
            </label>
            <div class="flex items-center gap-3">
                <button type="submit" class="btn">"Record"</button>
                {move || status.get().map(|s| view! { <span class="text-xs text-slate-400">{s}</span> })}
            </div>
        </form>
    }
}

#[component]
fn EventsTable(rows: Vec<CapitalEventDto>) -> impl IntoView {
    view! {
        <div class="tile overflow-x-auto">
            <table class="w-full text-sm">
                <thead>
                    <tr class="text-left text-xs uppercase text-slate-500">
                        <th class="py-2 pr-4 whitespace-nowrap">"When"</th>
                        <th class="py-2 pr-4">"Direction"</th>
                        <th class="py-2 pr-4">"Asset"</th>
                        <th class="py-2 pr-4 text-right">"Amount"</th>
                        <th class="py-2 pr-4 text-right">"USD at event"</th>
                        <th class="py-2 pr-4">"Notes"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|e| {
                        let amount_fmt = format_event_amount(&e.asset, &e.amount_atomic);
                        let usd_fmt = e.usd_value_at_event
                            .as_deref()
                            .map(format_usd_decimal)
                            .unwrap_or_else(|| "—".into());
                        view! {
                            <tr class="border-t border-slate-800">
                                <td class="py-2 pr-4 whitespace-nowrap">{e.occurred_at.format("%Y-%m-%d").to_string()}</td>
                                <td class="py-2 pr-4">{e.direction}</td>
                                <td class="py-2 pr-4">{e.asset}</td>
                                <td class="py-2 pr-4 text-right font-mono tabular-nums">{amount_fmt}</td>
                                <td class="py-2 pr-4 text-right font-mono tabular-nums">{usd_fmt}</td>
                                <td class="py-2 pr-4 text-slate-400">{e.notes.unwrap_or_default()}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

/// Convert an atomic-units amount to a human-friendly string given the
/// asset. BTC stored in sats (1e8); XMR stored in piconero (1e12); USD
/// stored in cents (1e2) for direct fiat-deposit rows.
fn format_event_amount(asset: &str, amount_atomic: &str) -> String {
    let raw: f64 = amount_atomic.parse().unwrap_or(0.0);
    match asset {
        "BTC" => format!("{:.8} BTC", raw / 1e8),
        "XMR" => format!("{:.6} XMR", raw / 1e12),
        "USD" => format!("${:.2}", raw / 100.0),
        _ => amount_atomic.to_string(),
    }
}

/// Drop trailing zeros after the decimal so $4570.00000000 reads $4570.00.
fn format_usd_decimal(raw: &str) -> String {
    let v: f64 = raw.parse().unwrap_or(0.0);
    format!("${:.2}", v)
}
