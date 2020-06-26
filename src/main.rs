use clap::{App, Arg};
// use futures::{Future, FutureExt, TryFutureExt};
// use juniper_subscriptions::Coordinator;
// use juniper_warp::subscriptions::graphql_subscriptions;
use slog::{info, o, Drain, Logger};
use snafu::ResultExt;
// use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqlitePool;
use std::net::ToSocketAddrs;
// use std::{pin::Pin, sync::Arc};
use warp::{self, Filter};

use delega::api::gql;
use delega::db;
use delega::error;

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    let matches = App::new("Microservice for driving data indexing")
        .version("0.1")
        .author("Matthieu Paindavoine")
        .arg(
            Arg::with_name("db_url")
                .value_name("STRING")
                .short("u")
                .long("db_url")
                .help("Connection String to database")
                .env("DATABASE_URL"),
        )
        .arg(
            Arg::with_name("address")
                .value_name("HOST")
                .short("h")
                .long("host")
                .default_value("localhost")
                .help("Address serving this server"),
        )
        .arg(
            Arg::with_name("port")
                .value_name("PORT")
                .short("p")
                .long("port")
                .default_value("8080")
                .help("Port"),
        )
        .arg(
            Arg::with_name("db")
                .value_name("STRING")
                .default_value("sqlite")
                .help("yourself"),
        )
        .get_matches();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());

    let db_url = matches
        .value_of("db_url")
        .ok_or_else(|| error::Error::MiscError {
            details: String::from("Could not get db_url"),
        })?;

    let addr = matches
        .value_of("address")
        .ok_or_else(|| error::Error::MiscError {
            details: String::from("Could not get address"),
        })?;

    let port = matches
        .value_of("port")
        .ok_or_else(|| error::Error::MiscError {
            details: String::from("Could not get port"),
        })?;

    let port = port.parse::<u16>().map_err(|err| error::Error::MiscError {
        details: format!("Could not parse into a valid port number ({})", err),
    })?;

    let db = matches
        .value_of("db")
        .ok_or_else(|| error::Error::MiscError {
            details: String::from("Could not get db"),
        })?;

    match db {
        "sqlite" => {
            run_server(
                (addr, port),
                logger,
                db::sqlite::connect(&db_url).await.context(error::DBError {
                    details: String::from("Conn"),
                })?,
            )
            .await
        }
        other => Err(error::Error::MiscError {
            details: format!("No support for '{}'", other),
        }),
    }?;

    Ok(())
}

async fn run_server(
    addr: impl ToSocketAddrs,
    logger: Logger,
    pool: SqlitePool,
) -> Result<(), error::Error> {
    let logger1 = logger.clone();
    let pool1 = pool.clone();
    let state = warp::any().map(move || gql::Context {
        pool: pool1.clone(),
        logger: logger1.clone(),
    });

    let graphiql = warp::path("graphiql")
        .and(warp::path::end())
        .and(warp::get())
        .and(juniper_warp::graphiql_filter("/graphql", None));

    let graphql_filter = juniper_warp::make_graphql_filter(gql::schema(), state.boxed());
    /* This is ApiRoutes.Base */
    let graphql = warp::path!("graphql").and(graphql_filter);

    let index = warp::fs::file("dist/index.html");

    let dir = warp::fs::dir("dist");

    // let routes = graphiql.or(graphql).or(notifications).or(dir).or(index);

    let routes = graphiql.or(graphql).or(dir).or(index);

    let addr = addr
        .to_socket_addrs()
        .context(error::IOError {
            details: String::from("To Sock Addr"),
        })?
        .next()
        .ok_or(error::Error::MiscError {
            details: String::from("Cannot resolve addr"),
        })?;

    info!(
        logger.clone(),
        "Serving Mjolnir on {}:{}",
        addr.ip(),
        addr.port()
    );
    warp::serve(routes).run(addr).await;

    Ok(())
}
