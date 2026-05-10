use leptos::prelude::*;

use crate::types::{HealthDto, SubsystemHealth};

#[server(name = GetHealth, prefix = "/api", endpoint = "health")]
pub async fn get_health() -> Result<HealthDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::health::fetch(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn HealthPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { get_health().await });

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-semibold">"Health"</h1>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || match data.get() {
                    None => view! { <div class="text-slate-400">"Loading…"</div> }.into_any(),
                    Some(Err(e)) => {
                        view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any()
                    }
                    Some(Ok(d)) => view! { <HealthGrid health=d/> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn HealthGrid(health: HealthDto) -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            <SubsystemCard name="asb" h=health.asb/>
            <SubsystemCard name="bitcoind" h=health.bitcoind/>
            <SubsystemCard name="monerod" h=health.monerod/>
            <SubsystemCard name="electrs" h=health.electrs/>
            <SubsystemCard name="tor" h=health.tor/>
            <SubsystemCard name="peers" h=health.peers/>
            <SubsystemCard name="rendezvous" h=health.rendezvous/>
            <SubsystemCard name="admin-db" h=health.admin_db/>
        </div>
        <p class="text-xs text-slate-500 mt-3">"As of " {health.as_of.to_rfc3339()}</p>
    }
}

#[component]
fn SubsystemCard(name: &'static str, h: SubsystemHealth) -> impl IntoView {
    let cls = h.state.badge_class();
    let state_label = format!("{:?}", h.state).to_lowercase();
    view! {
        <div class="tile">
            <div class="flex items-center justify-between">
                <div class="tile-title">{name}</div>
                <span class=cls>{state_label}</span>
            </div>
            <div class="tile-value text-base">{h.headline}</div>
            {h.detail.map(|d| view! { <div class="mt-1 text-xs text-slate-500">{d}</div> })}
        </div>
    }
}
