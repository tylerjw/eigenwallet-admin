use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};

use crate::components::nav::Nav;
use crate::pages::{
    charts::ChartsPage, competitors::CompetitorsPage, health::HealthPage, login::LoginPage,
    not_found::NotFoundPage, overview::OverviewPage, roi::RoiPage, spread::SpreadPage,
    swaps::SwapsPage, wallet_rules::WalletRulesPage,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en" class="dark">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <meta name="color-scheme" content="dark"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/eigenwallet-admin.css"/>
        <Title text="eigenwallet admin"/>
        <Router>
            <div class="min-h-screen flex flex-col">
                <Nav/>
                <main class="flex-1 max-w-7xl mx-auto w-full p-4 md:p-6">
                    <Routes fallback=NotFoundPage>
                        <Route path=StaticSegment("") view=OverviewPage/>
                        <Route path=StaticSegment("login") view=LoginPage/>
                        <Route path=StaticSegment("health") view=HealthPage/>
                        <Route path=StaticSegment("swaps") view=SwapsPage/>
                        <Route path=StaticSegment("charts") view=ChartsPage/>
                        <Route path=StaticSegment("spread") view=SpreadPage/>
                        <Route path=StaticSegment("competitors") view=CompetitorsPage/>
                        <Route path=StaticSegment("roi") view=RoiPage/>
                        <Route path=StaticSegment("wallet-rules") view=WalletRulesPage/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}
