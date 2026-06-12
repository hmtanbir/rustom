use serde::Deserialize;

/// Global application configuration loaded from environment variables.
#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    /// The host on which the Axum server will run.
    pub host: String,
    /// The port on which the Axum server will listen.
    pub port: u16,
    /// Connection string for PostgreSQL database.
    pub database_url: String,
    /// Connection string for Redis caching server.
    pub redis_url: String,
    /// Connection string for RabbitMQ messaging broker.
    pub rabbitmq_url: String,
    /// Secret key used to sign and verify JWT authentication tokens.
    pub jwt_secret: String,
    /// Duration in seconds for JWT tokens to remain valid.
    pub jwt_expiration_seconds: u64,
}

impl AppConfig {
    /// Load settings from environment variables and dotenv file.
    pub fn from_env() -> Result<Self, config::ConfigError> {
        // Determine application environment (default to "development")
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".into());
        println!("Loading configuration for environment: {}", app_env);

        // Try to load variables from environment-specific .env file
        let env_file = format!(".env.{}", app_env);
        if dotenvy::from_filename(&env_file).is_err() {
            // Fallback to default .env if environment-specific file is not found
            let _ = dotenvy::dotenv();
        }

        let builder = config::Config::builder()
            .set_default("host", "0.0.0.0")?
            .set_default("port", 8080)?
            .set_default("database_url", "postgres://postgres:postgres@localhost:5432/rustom_db")?
            .set_default("redis_url", "redis://127.0.0.1:6379/0")?
            .set_default("rabbitmq_url", "amqp://guest:guest@127.0.0.1:5672/%2f")?
            .set_default("jwt_secret", "super_secret_jwt_key_that_is_long_enough_to_be_secure_12345")?
            .set_default("jwt_expiration_seconds", 3600)?
            // Include values from system environment (overrides defaults and dotenv files)
            .add_source(config::Environment::default());

        builder.build()?.try_deserialize()
    }
}
