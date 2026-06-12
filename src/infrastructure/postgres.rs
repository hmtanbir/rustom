use sqlx::{postgres::PgPoolOptions, PgPool};
use crate::config::AppConfig;
use crate::errors::AppError;

/// Initialize and configure the PostgreSQL connection pool.
/// Automatically runs pending database migrations upon establishment.
pub async fn init_db(config: &AppConfig) -> Result<PgPool, AppError> {
    tracing::info!("Initializing PostgreSQL connection pool...");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .map_err(AppError::Database)?;

    tracing::info!("Applying pending migrations...");
    let mut migrator = sqlx::migrate!("./db/migrations");
    migrator.set_ignore_missing(true);
    let _: () = migrator.run(&pool)
        .await
        .map_err(|e| AppError::Unexpected(anyhow::anyhow!("Migration failed: {}", e)))?;

    tracing::info!("PostgreSQL database is ready.");
    Ok(pool)
}
