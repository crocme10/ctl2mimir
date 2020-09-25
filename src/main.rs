use clap::{App, Arg};
use futures::FutureExt;
use juniper_graphql_ws::ConnectionConfig;
use juniper_warp::{playground_filter, subscriptions::serve_graphql_ws};
use slog::{info, o, Drain, Logger};
use snafu::ResultExt;
use sqlx::sqlite::SqlitePool;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use warp::{self, Filter};

use ctl2mimir::api::gql;
use ctl2mimir::db;
use ctl2mimir::error;

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
    let playground = warp::get()
        .and(warp::path("playground"))
        .and(playground_filter("/graphql", Some("/subscriptions")));

    let logger1 = logger.clone();
    let pool1 = pool.clone();
    let qm_state1 = warp::any().map(move || gql::Context {
        pool: pool1.clone(),
        logger: logger1.clone(),
    });

    let qm_schema = gql::schema();
    let graphql = warp::post()
        .and(warp::path("graphql"))
        .and(juniper_warp::make_graphql_filter(
            qm_schema,
            qm_state1.boxed(),
        ));

    let root_node = Arc::new(gql::schema());

    let logger2 = logger.clone();
    let pool2 = pool.clone();
    let qm_state2 = warp::any().map(move || gql::Context {
        pool: pool2.clone(),
        logger: logger2.clone(),
    });

    let notifications = warp::path("subscriptions")
        .and(warp::ws())
        .and(qm_state2.clone())
        .map(move |ws: warp::ws::Ws, qm_state| {
            let root_node = root_node.clone();
            ws.on_upgrade(move |websocket| async move {
                serve_graphql_ws(websocket, root_node, ConnectionConfig::new(qm_state))
                    .map(|r| {
                        if let Err(e) = r {
                            println!("Websocket err: {}", e);
                        }
                    })
                    .await
            })
        })
        .map(|reply| warp::reply::with_header(reply, "Sec-Websocket-Protocol", "graphql-ws"));

    let index = warp::fs::file("dist/index.html");

    let dir = warp::fs::dir("dist");

    let routes = playground.or(graphql).or(notifications).or(dir).or(index);

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
