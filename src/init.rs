use clap::ArgMatches;
use config::Source;
use slog::{info, Logger};
use snafu::ResultExt;
use std::fs;

use mimir_ingest::db;
use mimir_ingest::error;
use mimir_ingest::settings::Settings;

#[allow(clippy::needless_lifetimes)]
pub async fn init<'a>(matches: &ArgMatches<'a>, logger: Logger) -> Result<(), error::Error> {
    info!(logger, "Initiazing application");
    let settings = Settings::new(matches)?;

    let _ = match fs::metadata(&settings.work.working_dir) {
        Err(_) => {
            info!(
                logger,
                "Creating working dir {}", &settings.work.working_dir
            );
            fs::create_dir(&settings.work.working_dir).context(error::IOError {
                details: format!(
                    "Could not create working dir {}",
                    &settings.work.working_dir
                ),
            })
        }
        Ok(metadata) => {
            if !metadata.is_dir() {
                Err(error::Error::MiscError {
                    details: format!("Working dir {} is a file", &settings.work.working_dir),
                })
            } else {
                Ok(())
            }
        }
    }?;

    if settings.debug {
        info!(logger, "Database URL: {}", settings.database.url);
    }

    // FIXME Here I hardcode, in the form of the path to the module, that we're using
    // a sqlite database...
    db::sqlite::init_db(&settings.database.url, logger).await
}
