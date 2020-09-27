use clap::ArgMatches;
use futures::FutureExt;
use juniper_graphql_ws::ConnectionConfig;
use juniper_warp::{playground_filter, subscriptions::serve_graphql_ws};
use slog::{info, Logger};
use snafu::ResultExt;
//use sqlx::sqlite::SqlitePool;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use warp::{self, Filter};

use ctl2mimir::api::gql;
use ctl2mimir::error;
use ctl2mimir::settings::Settings;
use ctl2mimir::state::State;

#[allow(clippy::needless_lifetimes)]
pub async fn run<'a>(matches: &ArgMatches<'a>, logger: Logger) -> Result<(), error::Error> {
    let settings = Settings::new(matches)?;
    let state = State::new(&settings, &logger).await?;
    run_server(settings, state).await
}

pub async fn run_server(settings: Settings, state: State) -> Result<(), error::Error> {
    // We keep a copy of the logger before the context takes ownership of it.
    let logger1 = state.logger.clone();
    let pool1 = state.pool.clone();
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

    let logger2 = state.logger.clone();
    let pool2 = state.pool.clone();
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

    let playground = warp::get()
        .and(warp::path("playground"))
        .and(playground_filter("/graphql", Some("/subscriptions")));

    let index = warp::fs::file("dist/index.html");

    let dir = warp::fs::dir("dist");

    let routes = playground.or(graphql).or(notifications).or(dir).or(index);

    let host = settings.service.host;
    let port = settings.service.port;
    let addr = (host.as_str(), port);
    let addr = addr
        .to_socket_addrs()
        .context(error::IOError {
            details: String::from("To Sock Addr"),
        })?
        .next()
        .ok_or(error::Error::MiscError {
            details: String::from("Cannot resolve addr"),
        })?;

    info!(state.logger, "Serving ctl2mimir");
    warp::serve(routes).run(addr).await;

    Ok(())
}
