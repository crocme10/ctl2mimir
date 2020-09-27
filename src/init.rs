use clap::ArgMatches;
use slog::{info, Logger};

use ctl2mimir::db;
use ctl2mimir::error;
use ctl2mimir::settings::Settings;

#[allow(clippy::needless_lifetimes)]
pub async fn init<'a>(matches: &ArgMatches<'a>, logger: Logger) -> Result<(), error::Error> {
    info!(logger, "Initiazing application");
    let settings = Settings::new(matches)?;

    info!(logger, "Mode: {}", settings.mode);

    if settings.debug {
        info!(logger, "Database URL: {}", settings.database.url);
    }

    // FIXME Here I hardcode, in the form of the path to the module, that we're using
    // a sqlite database...
    db::sqlite::init_db(&settings.database.url, logger).await
}
