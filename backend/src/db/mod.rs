//! PostgreSQL bağlantı havuzu ve migration çalıştırma.
//!
//! Pool, `DATABASE_URL`'den kurulur. Migration'lar derleme zamanında
//! `migrations/` dizininden gömülür ve başlangıçta forward-only uygulanır.

use sqlx::postgres::{PgPool, PgPoolOptions};

/// DB katmanı hataları.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database connection failed: {0}")]
    Connect(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// `DATABASE_URL` için bir bağlantı havuzu kurar (lazy bağlanır).
pub async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Bekleyen migration'ları uygular.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
