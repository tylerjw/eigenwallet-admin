use leptos::prelude::*;

use crate::types::{TaggedAddressDto, WalletRulesDto};

#[server(name = GetWalletRules, prefix = "/api", endpoint = "wallet-rules")]
pub async fn get_wallet_rules() -> Result<WalletRulesDto, ServerFnError> {
    let state = crate::server::ssr_state()?;
    crate::server::api::wallet_rules::fetch(&state)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn WalletRulesPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { get_wallet_rules().await });
    view! {
        <div class="space-y-4">
            <h1 class="text-2xl font-semibold">"Wallet rules"</h1>
            <div class="tile space-y-3">
                <p class="text-sm text-slate-300">
                    "Operator-managed registry of addresses we know about. Used to classify "
                    "wallet flow: an outgoing BTC tx to a "
                    <code class="bg-slate-800 px-1 rounded text-xs">"taker"</code>
                    " address is recycle flow, not a withdrawal; an incoming tx from a "
                    <code class="bg-slate-800 px-1 rounded text-xs">"cold-storage"</code>
                    " address is a capital deposit."
                </p>
                <p class="text-sm text-slate-300">
                    "This view is read-only. Edit by patching the "
                    <code class="bg-slate-800 px-1 rounded text-xs">"wallet-rules"</code>
                    " ConfigMap in the "
                    <code class="bg-slate-800 px-1 rounded text-xs">"eigenwallet"</code>
                    " namespace; changes are picked up within ~60 s without restarting the admin pod."
                </p>
                <pre class="text-xs bg-slate-900/80 border border-slate-800 rounded p-3 overflow-x-auto"><code>{
"kubectl edit configmap wallet-rules -n eigenwallet

# data.rules.yaml:
addresses:
  - addr: \"bc1q...\"        # the BTC address
    kind: taker              # taker | cold-storage | exchange | other
    asset: BTC               # BTC | XMR
    label: \"swap-cli taker BTC deposit\"
    note: \"internal recycle flow — not a capital event\"
  - addr: \"bc1q...\"
    kind: cold-storage
    asset: BTC
    label: \"main BTC vault\"
"
                }</code></pre>
            </div>
            <Suspense fallback=move || view! { <div class="text-slate-400">"Loading…"</div> }>
                {move || data.get().map(|res| match res {
                    Ok(r) => view! { <RulesView r=r/> }.into_any(),
                    Err(e) => view! { <div class="tile text-rose-300">{e.to_string()}</div> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn RulesView(r: WalletRulesDto) -> impl IntoView {
    let count = r.addresses.len();
    view! {
        <div class="tile space-y-3">
            <div class="flex items-baseline gap-3 flex-wrap text-xs text-slate-500">
                <span>{format!("{} entries", count)}</span>
                {r.last_loaded.map(|t| view! {
                    <span>"last refresh " {t.format("%Y-%m-%d %H:%M UTC").to_string()}</span>
                })}
                {r.last_error.map(|e| view! {
                    <span class="text-rose-400">"refresh error: " {e}</span>
                })}
            </div>
            {if count == 0 {
                view! {
                    <div class="text-sm text-slate-400">
                        "No rules configured. The ConfigMap either doesn't exist or is empty — "
                        "create it with the snippet above to start classifying wallet flow."
                    </div>
                }.into_any()
            } else {
                view! { <RulesTable rows=r.addresses/> }.into_any()
            }}
        </div>
    }
}

#[component]
fn RulesTable(rows: Vec<TaggedAddressDto>) -> impl IntoView {
    view! {
        <div class="overflow-x-auto">
            <table class="w-full text-sm">
                <thead>
                    <tr class="text-left text-xs uppercase text-slate-500">
                        <th class="py-2 pr-4">"Kind"</th>
                        <th class="py-2 pr-4">"Asset"</th>
                        <th class="py-2 pr-4">"Address"</th>
                        <th class="py-2 pr-4">"Label"</th>
                        <th class="py-2 pr-4">"Note"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|e| {
                        let cls = match e.kind.as_str() {
                            "taker" => "badge-warn",
                            "cold-storage" => "badge-ok",
                            "exchange" => "badge-warn",
                            _ => "badge-warn",
                        };
                        let addr_short = if e.addr.len() > 24 {
                            format!("{}…{}", &e.addr[..10], &e.addr[e.addr.len() - 10..])
                        } else {
                            e.addr.clone()
                        };
                        view! {
                            <tr class="border-t border-slate-800">
                                <td class="py-2 pr-4"><span class=cls>{e.kind}</span></td>
                                <td class="py-2 pr-4">{e.asset.unwrap_or_else(|| "—".into())}</td>
                                <td class="py-2 pr-4 font-mono text-xs" title=e.addr.clone()>{addr_short}</td>
                                <td class="py-2 pr-4">{e.label.unwrap_or_default()}</td>
                                <td class="py-2 pr-4 text-slate-400">{e.note.unwrap_or_default()}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
