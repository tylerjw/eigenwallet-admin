#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    eigenwallet_admin::server::run().await;
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
