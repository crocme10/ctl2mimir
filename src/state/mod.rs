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
        // Here i need to do a little pirouette...
        // movine does not support (yet) DATABASE_URL. So the content of settings.database.url
        // is actually not a url, but just a filename.
        // sqlx, on the other hand, takes a database_url.... so that's what i'll build.
        let database_url = format!("sqlite://{}", &settings.database.url);
        let pool = SqlitePool::builder()
            .max_size(5)
            .build(&database_url)
            .await
            .context(error::DBError {
                details: String::from("foo"),
            })?;
        // FIXME ping the pool to know quickly if we have a db connection

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
