//! Thin convenience wrappers over the pool.

use anyhow::Result;
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::pooled_connection::bb8::PooledConnection;

pub type Conn<'a> = PooledConnection<'a, AsyncPgConnection>;

pub async fn checkout<'a>(pool: &'a Pool<AsyncPgConnection>) -> Result<Conn<'a>> {
    pool.get()
        .await
        .map_err(|e| anyhow::anyhow!("db checkout: {e}"))
}
