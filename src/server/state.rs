use std::sync::Arc;

use axum::extract::FromRef;
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::bb8::Pool;
use leptos::config::LeptosOptions;
use tokio::sync::RwLock;

use crate::server::clients::asb::AsbClient;
use crate::server::clients::cex::CexCache;
use crate::server::clients::electrs::ElectrsClient;
use crate::server::clients::monerod::MonerodClient;
use crate::server::kube_client::KubeClient;
use crate::server::wallet_rules::{WalletRulesCache, WalletRulesHandle};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub asb_rpc_url: String,
    pub monerod_rpc_url: String,
    pub electrs_url: String,
    pub asb_namespace: String,
    pub asb_deployment_name: String,
    pub asb_configmap_name: String,
    pub swap_cli_image: String,
    pub asb_log_dir: String,
    pub our_peer_id: Option<String>,
    pub rendezvous_points: Vec<String>,
    pub session_secret: String,
    pub site_addr: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        fn env_or(k: &str, default: &str) -> String {
            std::env::var(k).unwrap_or_else(|_| default.to_string())
        }
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL"),
            asb_rpc_url: env_or("ASB_RPC_URL", "http://asb:9944"),
            monerod_rpc_url: env_or("MONEROD_RPC_URL", "http://monerod:18081"),
            electrs_url: env_or("ELECTRS_URL", "tcp://electrs:50001"),
            asb_namespace: env_or("ASB_NAMESPACE", "eigenwallet"),
            asb_deployment_name: env_or("ASB_DEPLOYMENT_NAME", "asb"),
            asb_configmap_name: env_or("ASB_CONFIGMAP_NAME", "asb-config"),
            swap_cli_image: env_or(
                "SWAP_CLI_IMAGE",
                "ghcr.io/tylerjw/eigenwallet-swap-cli:4.5.0",
            ),
            asb_log_dir: env_or("ASB_LOG_DIR", "/asb-data/logs"),
            our_peer_id: std::env::var("OUR_PEER_ID").ok(),
            rendezvous_points: std::env::var("RENDEZVOUS_POINTS")
                .ok()
                .map(|s| {
                    s.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            session_secret: std::env::var("SESSION_SECRET").unwrap_or_else(|_| {
                tracing::warn!("SESSION_SECRET not set — sessions won't survive restart");
                // 64-byte random fallback
                use rand::RngCore;
                let mut bytes = [0u8; 64];
                rand::rngs::OsRng.fill_bytes(&mut bytes);
                hex_lower(&bytes)
            }),
            site_addr: env_or("SITE_ADDR", "0.0.0.0:4000"),
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub struct AppStateInner {
    pub config: AppConfig,
    pub pool: Pool<AsyncPgConnection>,
    pub asb: AsbClient,
    pub monerod: MonerodClient,
    pub electrs: ElectrsClient,
    pub cex: RwLock<CexCache>,
    pub kube: Option<KubeClient>,
    pub wallet_rules: WalletRulesHandle,
    pub leptos_options: LeptosOptions,
}

#[derive(Clone)]
pub struct AppState(pub Arc<AppStateInner>);

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &AppStateInner {
        &self.0
    }
}

impl AppState {
    pub async fn new(config: AppConfig, pool: Pool<AsyncPgConnection>) -> Self {
        let asb = AsbClient::new(&config.asb_rpc_url);
        let monerod = MonerodClient::new(&config.monerod_rpc_url);
        let electrs = ElectrsClient::new(&config.electrs_url);
        let kube = match KubeClient::try_in_cluster().await {
            Ok(k) => Some(k),
            Err(e) => {
                tracing::warn!(error = %e, "kube client init failed — k8s-dependent features disabled");
                None
            }
        };
        let leptos_options = leptos::config::get_configuration(None)
            .map(|c| c.leptos_options)
            .unwrap_or_else(|_| {
                LeptosOptions::builder()
                    .output_name("eigenwallet-admin")
                    .build()
            });
        Self(Arc::new(AppStateInner {
            config,
            pool,
            asb,
            monerod,
            electrs,
            cex: RwLock::new(CexCache::default()),
            kube,
            wallet_rules: Arc::new(RwLock::new(WalletRulesCache::default())),
            leptos_options,
        }))
    }
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}
