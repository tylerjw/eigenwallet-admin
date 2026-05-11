use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Nav() -> impl IntoView {
    view! {
        <header class="border-b border-slate-800 bg-slate-900/80 backdrop-blur sticky top-0 z-10">
            <div class="max-w-7xl mx-auto px-4 md:px-6 py-3 flex items-center gap-4 overflow-x-auto">
                <A href="/" attr:class="font-semibold text-slate-100 whitespace-nowrap">
                    "eigenwallet admin"
                </A>
                <NavLink href="/" label="Overview"/>
                <NavLink href="/health" label="Health"/>
                <NavLink href="/swaps" label="Swaps"/>
                <NavLink href="/charts" label="Charts"/>
                <NavLink href="/spread" label="Spread"/>
                <NavLink href="/competitors" label="Competitors"/>
                <NavLink href="/roi" label="ROI"/>
                <NavLink href="/wallet-rules" label="Wallet rules"/>
                <form method="POST" action="/api/auth/logout" class="ml-auto">
                    <button
                        type="submit"
                        class="text-sm text-slate-400 hover:text-slate-200 bg-transparent border-0 cursor-pointer"
                    >
                        "Logout"
                    </button>
                </form>
            </div>
        </header>
    }
}

#[component]
fn NavLink(href: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <A href=href attr:class="text-sm text-slate-300 hover:text-slate-100 whitespace-nowrap">
            {label}
        </A>
    }
}
