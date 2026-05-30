//! SQLite-backed credential and device store.
//!
//! Passwords are hashed with Argon2. Credentials are seeded out of band via the
//! `seed-user` CLI subcommand; the `/api/login` handler only ever reads them.

use std::str::FromStr;

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

/// A device record (created on first successful login for a credential).
#[derive(Debug, Clone)]
pub struct Device {
    pub device_id: String,
    pub mqtt_username: String,
}

impl Store {
    /// Open (creating the file if absent), run migrations, and return the store.
    pub async fn connect(database_url: &str) -> Result<Self, String> {
        let opts = SqliteConnectOptions::from_str(database_url)
            .map_err(|e| format!("invalid database url '{database_url}': {e}"))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .map_err(|e| format!("failed to open sqlite db: {e}"))?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| format!("migration failed: {e}"))?;

        Ok(Self { pool })
    }

    /// Insert or replace a credential, hashing `password` with Argon2.
    pub async fn upsert_credential(&self, username: &str, password: &str) -> Result<(), String> {
        let pw_hash = hash_password(password)?;
        let now = unix_now();
        sqlx::query(
            "INSERT INTO credentials (username, pw_hash, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(username) DO UPDATE SET pw_hash = excluded.pw_hash",
        )
        .bind(username)
        .bind(pw_hash)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("failed to upsert credential: {e}"))?;
        Ok(())
    }

    /// Verify a username/password pair against the store.
    pub async fn verify_credential(&self, username: &str, password: &str) -> Result<bool, String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT pw_hash FROM credentials WHERE username = ?1")
                .bind(username)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| format!("failed to read credential: {e}"))?;

        let Some((pw_hash,)) = row else { return Ok(false) };
        Ok(verify_password(&pw_hash, password))
    }

    /// Return the device for a username, creating a fresh one if none exists.
    /// The `mqtt_username` is the `device_id` (the broker identity).
    pub async fn get_or_create_device(&self, username: &str) -> Result<Device, String> {
        if let Some(dev) = self.find_device(username).await? {
            return Ok(dev);
        }

        let device_id = uuid::Uuid::new_v4().to_string();
        let now = unix_now();
        // INSERT OR IGNORE guards the race where two logins create concurrently;
        // we re-read afterwards so both callers converge on the same row.
        sqlx::query(
            "INSERT OR IGNORE INTO devices (device_id, username, mqtt_username, created_at)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&device_id)
        .bind(username)
        .bind(&device_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("failed to create device: {e}"))?;

        self.find_device(username)
            .await?
            .ok_or_else(|| "device disappeared after insert".to_string())
    }

    async fn find_device(&self, username: &str) -> Result<Option<Device>, String> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT device_id, mqtt_username FROM devices WHERE username = ?1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("failed to read device: {e}"))?;

        Ok(row.map(|(device_id, mqtt_username)| Device { device_id, mqtt_username }))
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("failed to hash password: {e}"))
}

fn verify_password(pw_hash: &str, password: &str) -> bool {
    match PasswordHash::new(pw_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}
