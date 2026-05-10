use leptos::prelude::*;

use crate::types::{CapitalEventDto, CapitalEventInput, RoiDto};

#[server(name = GetRoi, prefix = "/api", endpoint = "roi")]
pub async fn get_roi(
    since: Option<String>,
    method: Option<String>,
    denom: Option<String>,
) -> Result<RoiDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::roi::compute(
        &state,
        since.as_deref(),
        method.as_deref().unwrap_or("mtm"),
        denom.as_deref().unwrap_or("usd"),
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = ListCapitalEvents, prefix = "/api", endpoint = "capital-events")]
pub async fn list_capital_events() -> Result<Vec<CapitalEventDto>, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::capital::list(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(name = AddCapitalEvent, prefix = "/api", endpoint = "capital-events")]
pub async fn add_capital_event(input: CapitalEventInput) -> Result<CapitalEventDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::capital::add(&state, input)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn RoiPage() -> impl IntoView {
    let roi = Resource::new(|| (), |_| async move { get_roi(None, None, None).await });
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
fn RoiTile(r: RoiDto) -> impl IntoView {
    view! {
        <div class="tile">
            <div class="tile-title">"Return (" {r.method.clone()} ", " {r.denomination.clone()} ")"</div>
            <div class="tile-value">{format!("{}%", r.pct_change)}</div>
            <div class="text-xs text-slate-500 mt-1">
                {format!(
                    "from {} ({} days): {} → {}",
                    r.since.format("%Y-%m-%d"),
                    r.days_elapsed,
                    r.start_value,
                    r.current_value
                )}
            </div>
        </div>
    }
}

#[component]
fn CapitalEventForm(on_added: impl Fn() + 'static + Clone + Send) -> impl IntoView {
    let direction = RwSignal::new("deposit".to_string());
    let asset = RwSignal::new("BTC".to_string());
    let amount = RwSignal::new(String::new());
    let usd = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let status = RwSignal::new(Option::<String>::None);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let input = CapitalEventInput {
            occurred_at: chrono::Utc::now(),
            direction: direction.get(),
            asset: asset.get(),
            amount_atomic: amount.get(),
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
                    on_added();
                }
                Err(e) => status.set(Some(format!("FAIL: {e}"))),
            }
        });
    };

    view! {
        <form class="tile space-y-3" on:submit=on_submit>
            <div class="tile-title">"Record capital event"</div>
            <div class="grid grid-cols-2 md:grid-cols-5 gap-3">
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
                    "Amount (atomic units)"
                    <input
                        class="input mt-1"
                        prop:value=move || amount.get()
                        on:input=move |ev| amount.set(event_target_value(&ev))
                    />
                </label>
                <label class="text-xs uppercase text-slate-400">
                    "USD override (optional)"
                    <input
                        class="input mt-1"
                        prop:value=move || usd.get()
                        on:input=move |ev| usd.set(event_target_value(&ev))
                    />
                </label>
                <label class="text-xs uppercase text-slate-400">
                    "Notes"
                    <input
                        class="input mt-1"
                        prop:value=move || notes.get()
                        on:input=move |ev| notes.set(event_target_value(&ev))
                    />
                </label>
            </div>
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
                        <th class="py-2 pr-4">"When"</th>
                        <th class="py-2 pr-4">"Direction"</th>
                        <th class="py-2 pr-4">"Asset"</th>
                        <th class="py-2 pr-4">"Amount"</th>
                        <th class="py-2 pr-4">"USD at event"</th>
                        <th class="py-2 pr-4">"Notes"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|e| {
                        view! {
                            <tr class="border-t border-slate-800">
                                <td class="py-2 pr-4">{e.occurred_at.format("%Y-%m-%d").to_string()}</td>
                                <td class="py-2 pr-4">{e.direction}</td>
                                <td class="py-2 pr-4">{e.asset}</td>
                                <td class="py-2 pr-4">{e.amount_atomic}</td>
                                <td class="py-2 pr-4">{e.usd_value_at_event.unwrap_or_else(|| "—".into())}</td>
                                <td class="py-2 pr-4 text-slate-400">{e.notes.unwrap_or_default()}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
