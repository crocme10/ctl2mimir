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
    run_server(state).await
}

pub async fn run_server(state: State) -> Result<(), error::Error> {
    // We keep a copy of the logger before the context takes ownership of it.
    let state1 = state.clone();
    let qm_state1 = warp::any().map(move || gql::Context {
        state: state1.clone(),
    });

    let qm_schema = gql::schema();
    let graphql = warp::post()
        .and(warp::path("graphql"))
        .and(juniper_warp::make_graphql_filter(
            qm_schema,
            qm_state1.boxed(),
        ));

    let root_node = Arc::new(gql::schema());

    let state2 = state.clone();
    let qm_state2 = warp::any().map(move || gql::Context {
        state: state2.clone(),
    });

    let notifications = warp::path("subscriptions")
        .and(warp::ws())
        .and(qm_state2.clone())
        .map(move |ws: warp::ws::Ws, qm_state| {
            let root_node = root_node.clone();
            ws.on_upgrade(move |websocket| {
                async move {
                    serve_graphql_ws(websocket, root_node, ConnectionConfig::new(qm_state))
                        .map(|r| {
                            if let Err(e) = r {
                                println!("Websocket err: {}", e);
                            }
                        })
                        .await
                }
            })
        })
        .map(|reply| warp::reply::with_header(reply, "Sec-Websocket-Protocol", "graphql-ws"));

    let playground = warp::get()
        .and(warp::path("playground"))
        .and(playground_filter("/graphql", Some("/subscriptions")));

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST"])
        .allow_headers(vec!["content-type", "authorization"])
        .allow_any_origin()
        .build();

    let log = warp::log("foo");

    let routes = playground
        .or(graphql)
        .or(notifications)
        .with(cors)
        .with(log);

    let host = state.settings.service.host;
    let port = state.settings.service.port;
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
