use lapin::{
    options::QueueDeclareOptions,
    types::FieldTable,
    Channel, Connection, ConnectionProperties,
};
use crate::config::AppConfig;
use crate::domain::AppError;

/// Name of the message queue for processing background tasks.
pub const JOBS_QUEUE: &str = "rustom_jobs_queue";

/// Initialize RabbitMQ connection and channel, declaring the target queue.
pub async fn init_rabbitmq(config: &AppConfig) -> Result<(Connection, Channel), AppError> {
    tracing::info!("Initializing RabbitMQ connection at {}...", config.rabbitmq_url);

    let conn = Connection::connect(
        &config.rabbitmq_url,
        ConnectionProperties::default(),
    )
    .await
    .map_err(AppError::Queue)?;

    let channel = conn.create_channel().await.map_err(AppError::Queue)?;

    tracing::info!("Declaring queue: {}", JOBS_QUEUE);
    channel
        .queue_declare(
            JOBS_QUEUE,
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await
        .map_err(AppError::Queue)?;

    tracing::info!("RabbitMQ is ready.");
    Ok((conn, channel))
}
