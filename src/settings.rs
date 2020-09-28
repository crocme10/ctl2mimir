use clap::ArgMatches;
use config::{Config, Environment, File};
use serde::Deserialize;
use snafu::ResultExt;
use std::env;

use super::error;

#[derive(Debug, Clone, Deserialize)]
pub struct Zmq {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Elasticsearch {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Work {
    pub working_dir: String,
    pub mimirsbrunn_dir: String,
    pub cosmogony_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Database {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Service {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub debug: bool,
    pub testing: bool,
    pub mode: String,
    pub database: Database,
    pub service: Service,
    pub zmq: Zmq,
    pub elasticsearch: Elasticsearch,
    pub work: Work,
}

// TODO Parameterize the config directory

impl Settings {
    pub fn new<'a, T: Into<Option<&'a ArgMatches<'a>>>>(matches: T) -> Result<Self, error::Error> {
        let mut s = Config::new();

        // Start off by merging in the "default" configuration file
        s.merge(File::with_name("config/default"))
            .context(error::ConfigError {
                details: String::from("Could not merge default configuration"),
            })?;

        // Add in the current environment file
        // Default to 'development' env
        // Note that this file is _optional_
        let mode = env::var("RUN_MODE").unwrap_or_else(|_| String::from("development"));
        s.merge(File::with_name(&format!("config/{}", mode)).required(true))
            .context(error::ConfigError {
                details: format!("Could not merge {} configuration", mode),
            })?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        s.merge(File::with_name("config/local").required(false))
            .context(error::ConfigError {
                details: String::from("Could not merge local configuration"),
            })?;

        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("app"))
            .context(error::ConfigError {
                details: String::from("Could not merge configuration from environment variables"),
            })?;

        // Now we take care of the database.url, which can be had from environment variables.
        let key = match mode.as_str() {
            "testing" => "SQLITE_TEST_FILE",
            _ => "SQLITE_FILE",
        };

        let db_url = env::var(key).context(error::EnvVarError {
            details: format!("Could not get env var {}", key),
        })?;

        s.set("database.url", db_url).context(error::ConfigError {
            details: String::from("Could not set database url from environment variable"),
        })?;

        let m = matches.into();
        if let Some(m) = m {
            // Finally we override values with what has been given at the command line
            if let Some(addr) = m.value_of("address") {
                s.set("service.host", addr).context(error::ConfigError {
                    details: String::from("Could not set service host from CLI argument"),
                })?;
            }

            if let Some(port) = m.value_of("port") {
                let _port = port.parse::<u16>().map_err(|err| error::Error::MiscError {
                    details: format!("Could not parse into a valid port number ({})", err),
                })?;
                s.set("service.port", port).context(error::ConfigError {
                    details: String::from("Could not set service port from CLI argument"),
                })?;
            }
        }

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_into().context(error::ConfigError {
            details: String::from("Could not generate settings from configuration"),
        })
    }
}
