//! Server-only modules. Behind the `ssr` feature.

pub mod api;
pub mod auth;
pub mod clients;
pub mod db;
pub mod kube_client;
pub mod models;
pub mod pollers;
pub mod schema;
pub mod state;
pub mod wallet_rules;

use std::sync::Arc;

use axum::Router;
use diesel::prelude::*;
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use leptos::config::get_configuration;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use tower_sessions::{MemoryStore, SessionManagerLayer, cookie::Key};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub use state::{AppConfig, AppState};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub async fn run() {
    // rustls 0.23 panics on first TLS use unless a CryptoProvider is installed.
    // Several deps (reqwest, kube, jsonrpsee) bring rustls 0.23 transitively
    // and none of them feature-enable a provider, so we install ring here.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install rustls ring crypto provider");

    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,eigenwallet_admin=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    tracing::info!(asb = %config.asb_rpc_url, "config loaded");

    // Migrate
    {
        let mut conn = PgConnection::establish(&config.database_url)
            .expect("connect to admin db for migrations");
        conn.run_pending_migrations(MIGRATIONS)
            .expect("apply migrations");
        tracing::info!("migrations applied");
    }

    let mgr = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&config.database_url);
    let pool = Pool::builder().build(mgr).await.expect("build db pool");

    let state = AppState::new(config.clone(), pool).await;

    // Background pollers
    pollers::spawn_all(state.clone());

    // Leptos / axum wiring (axum is the underlying transport for Leptos SSR;
    // all REST endpoints are Leptos server functions auto-mounted under /api).
    let leptos_options = get_configuration(None)
        .expect("leptos config")
        .leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(crate::app::App);

    let session_store = MemoryStore::default();
    let session_key = Key::from(state.config.session_secret.as_bytes());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_signed(session_key)
        .with_name("ewa_session");

    let app: Router = Router::new()
        .leptos_routes_with_context(
            &state,
            routes,
            {
                let s = state.clone();
                move || provide_context(s.clone())
            },
            {
                let opts = leptos_options.clone();
                move || crate::app::shell(opts.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(
            crate::app::shell,
        ))
        .layer(axum::middleware::from_fn(auth_gate))
        .layer(session_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("serve");
}

/// Extract the AppState inside a server function.
pub fn ssr_state() -> Result<Arc<state::AppStateInner>, leptos::server_fn::ServerFnError> {
    use_context::<AppState>()
        .map(|s| s.0)
        .ok_or_else(|| leptos::server_fn::ServerFnError::new("missing app state in context"))
}

/// Redirect unauthenticated users to /login unless they're requesting a public path.
async fn auth_gate(
    session: tower_sessions::Session,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::{IntoResponse, Redirect};
    let path = req.uri().path();
    let public = path == "/login"
        || path.starts_with("/api/auth/")
        || path.starts_with("/pkg/")
        || path.starts_with("/assets/")
        || path == "/favicon.ico";
    if public {
        return next.run(req).await;
    }
    if auth::is_authed(&session).await {
        return next.run(req).await;
    }
    if path.starts_with("/api/") {
        // Server-function call: respond with 401 so the client can surface it.
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    Redirect::to("/login").into_response()
}
