use crate::error;
use crate::settings::Settings;
use slog::{info, o, Logger};
use snafu::ResultExt;
use sqlx::prelude::SqliteQueryAs;
use sqlx::sqlite::SqlitePool;

#[derive(Clone, Debug)]
pub struct State {
    pub pool: SqlitePool,
    pub logger: Logger,
    pub settings: Settings,
}

impl State {
    pub async fn new(settings: &Settings, logger: &Logger) -> Result<Self, error::Error> {
        let database_url = format!(
            "sqlite:{}",
            settings.database.url.trim_start_matches("sqlite://")
        );
        info!(logger, "Setting up state with db {}", database_url);
        let pool = SqlitePool::builder()
            .max_size(5)
            .build(&database_url)
            .await
            .context(error::DBError {
                details: String::from("foo"),
            })?;

        let row: (String,) = sqlx::query_as("SELECT sqlite_version()")
            .fetch_one(&pool)
            .await
            .context(error::DBError {
                details: format!("Could not test database version for {}", &database_url,),
            })?;

        info!(logger, "db version: {:?}", row.0);

        // I make a quick connection check with elasticsearch, cause what's the point
        // of continuing if we don't have no elasticsearch...
        let elasticsearch_endpoint = format!(
            "http://{}:{}",
            settings.elasticsearch.host, settings.elasticsearch.port
        );
        let _body =
            reqwest::blocking::get(&elasticsearch_endpoint).context(error::ReqwestError {
                details: format!(
                    "Failed to connect to elasticsearch at '{}'",
                    &elasticsearch_endpoint
                ),
            })?;

        let logger = logger.new(
            o!("host" => String::from(&settings.service.host), "port" => settings.service.port, "database" => String::from(&settings.database.url)),
        );

        Ok(Self {
            pool,
            logger,
            settings: settings.clone(),
        })
    }
}
