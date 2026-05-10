use anyhow::{Result, anyhow};
use argon2::Argon2;
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use tower_sessions::Session;

use crate::server::db;
use crate::server::models::AdminCredential;
use crate::server::schema::admin_credentials;
use crate::server::state::AppStateInner;

const SESSION_AUTHED_KEY: &str = "authed";

pub async fn is_authed(session: &Session) -> bool {
    matches!(
        session.get::<bool>(SESSION_AUTHED_KEY).await,
        Ok(Some(true))
    )
}

pub async fn mark_authed(session: &Session) -> Result<()> {
    session
        .insert(SESSION_AUTHED_KEY, true)
        .await
        .map_err(|e| anyhow!("session insert: {e}"))?;
    Ok(())
}

pub async fn clear(session: &Session) {
    let _ = session.remove::<bool>(SESSION_AUTHED_KEY).await;
}

pub async fn verify_password(state: &AppStateInner, password: &str) -> Result<bool> {
    let mut conn = db::checkout(&state.pool).await?;
    let row: Option<AdminCredential> = admin_credentials::table
        .select(AdminCredential::as_select())
        .first(&mut *conn)
        .await
        .optional()?;
    let Some(row) = row else {
        return Ok(false);
    };
    let parsed = PasswordHash::new(&row.password_hash).map_err(|e| anyhow!("parse hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("hash: {e}"))?;
    Ok(hash.to_string())
}

pub async fn set_password(state: &AppStateInner, password: &str) -> Result<()> {
    let hash = hash_password(password)?;
    let mut conn = db::checkout(&state.pool).await?;
    let existing: Option<AdminCredential> = admin_credentials::table
        .select(AdminCredential::as_select())
        .first(&mut *conn)
        .await
        .optional()?;
    if existing.is_some() {
        diesel::update(admin_credentials::table)
            .set((
                admin_credentials::password_hash.eq(hash),
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
