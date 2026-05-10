//! CLI to set or rotate the single admin password hash.
//!
//! Usage (in-cluster):
//!   kubectl exec -it deploy/admin -- seed-admin set-password
//! Reads password from stdin (no echo).

use std::io::{BufRead, Write};

use clap::{Parser, Subcommand};
use diesel::Connection;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Set or rotate the admin password.
    SetPassword {
        /// Read password from this env var instead of stdin (for non-interactive use).
        #[arg(long)]
        from_env: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;

    {
        let mut conn = PgConnection::establish(&database_url)?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| anyhow::anyhow!("migrations: {e}"))?;
    }

    let mgr = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&database_url);
    let pool = Pool::builder().build(mgr).await?;

    match cli.cmd {
        Cmd::SetPassword { from_env } => {
            let password = match from_env {
                Some(var) => {
                    std::env::var(&var).map_err(|_| anyhow::anyhow!("env var {var} not set"))?
                }
                None => {
                    eprint!("New admin password: ");
                    std::io::stderr().flush().ok();
                    let mut line = String::new();
                    std::io::stdin().lock().read_line(&mut line)?;
                    let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
                    if trimmed.len() < 12 {
                        anyhow::bail!("password must be at least 12 chars");
                    }
                    trimmed
                }
            };
            let hash = eigenwallet_admin::server::auth::hash_password(&password)?;
            store_hash(&pool, hash).await?;
            eprintln!("admin password set");
        }
    }

    Ok(())
}

async fn store_hash(pool: &Pool<AsyncPgConnection>, hash: String) -> anyhow::Result<()> {
    use diesel::SelectableHelper;
    use diesel::query_dsl::methods::SelectDsl;
    use diesel_async::RunQueryDsl;
    use eigenwallet_admin::server::models::AdminCredential;
    use eigenwallet_admin::server::schema::admin_credentials;

    let mut conn = pool.get().await?;
    let existing: Option<AdminCredential> =
        SelectDsl::select(admin_credentials::table, AdminCredential::as_select())
            .first(&mut *conn)
            .await
            .optional()?;
    if existing.is_some() {
        diesel::update(admin_credentials::table)
            .set((
                admin_credentials::password_hash.eq(&hash),
                admin_credentials::rotated_at.eq(Some(chrono::Utc::now())),
            ))
            .execute(&mut *conn)
            .await?;
    } else {
        let new = AdminCredential {
            id: uuid::Uuid::new_v4(),
            password_hash: hash,
            created_at: chrono::Utc::now(),
            rotated_at: None,
        };
        diesel::insert_into(admin_credentials::table)
            .values(&new)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}
