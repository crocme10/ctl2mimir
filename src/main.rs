use clap::{App, Arg};
use futures::{Future, FutureExt, TryFutureExt};
use juniper_subscriptions::Coordinator;
use juniper_warp::subscriptions::graphql_subscriptions;
use slog::{error, info, o, Drain, Logger};
use std::{pin::Pin, sync::Arc};
use warp::{self, Filter};

use crate::api::gql;
use crate::db::{self, model::ProvideData, Db};
use crate::error;

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    let matches = App::new("Microservice for driving data indexing")
        .version("0.1")
        .author("Matthieu Paindavoine")
        .arg(
            Arg::with_name("db_url")
                .value_name("STRING")
                .help("Connection String to database")
                .env("DATABASE_URL"),
        )
        .arg(
            Arg::with_name("address")
                .value_name("HOST")
                .default("localhost")
                .help("Address serving this server"),
        )
        .arg(
            Arg::with_name("port")
                .value_name("PORT")
                .default("8080")
                .help("Port"),
        )
        .arg(
            Arg::with_name("db")
                .value_name("STRING")
                .default("sqlite")
                .help("yourself"),
        )
        .get_matches();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let log = slog::Logger::root(drain, o!());

    let db_url = matches
        .value_of("db_url")
        .ok_or_else(|| error::Error::UserError {
            details: String::from("Could not get db_url"),
        })?;

    let address = matches
        .value_of("address")
        .ok_or_else(|| error::Error::UserError {
            details: String::from("Could not get address"),
        })?;

    let port = matches
        .value_of("address")
        .ok_or_else(|| error::Error::UserError {
            details: String::from("Could not get port"),
        })?;

    let port = port.parse::<u16>().context(error::Error::UserError {
        details: String::from("Could not parse into a valid port number"),
    })?;

    let db = matches
        .value_of("db")
        .ok_or_else(|| error::Error::UserError {
            details: String::from("Could not get db"),
        })?;

    let addr = (address.as_str(), port);

    match db.as_str() {
        "sqlite" => run_server(addr, logger, db::sqlite::connect(&db_url).await?).await,
        other => Err(error::Error::UserError {
            details: String::from("No support for '{}'", other),
        }),
    }?;

    Ok(())
}

async fn run_server<S, C>(
    addr: impl ToSocketAddrs,
    logger: Logger,
    pool: S,
) -> Result<(), error::Error>
where
    S: Send + Sync + Db<Conn = PoolConnection<C>> + 'static,
    C: sqlx::Connect + ProvideData,
{
    let logger1 = root_logger.clone();
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

    // let logger2 = root_logger.clone();
    // let pool2 = pool.clone();
    // let substate = warp::any().map(move || gql::Context {
    //     pool: pool2.clone(),
    //     logger: logger2.clone(),
    // });

    // let coordinator = Arc::new(juniper_subscriptions::Coordinator::new(gql::schema()));

    // let notifications = (warp::path("notifications")
    //     .and(warp::ws())
    //     .and(substate.clone())
    //     .and(warp::any().map(move || Arc::clone(&coordinator)))
    //     .map(
    //         |ws: warp::ws::Ws,
    //          context: gql::Context,
    //          coordinator: Arc<Coordinator<'static, _, _, _, _, _>>| {
    //             ws.on_upgrade(|websocket| -> Pin<Box<dyn Future<Output = ()> + Send>> {
    //                 println!("On upgrade");
    //                 graphql_subscriptions(websocket, coordinator, context)
    //                     .map(|r| {
    //                         println!("r: {:?}", r);
    //                         if let Err(err) = r {
    //                             println!("Websocket Error: {}", err);
    //                         }
    //                     })
    //                     .boxed()
    //             })
    //         },
    //     ))
    // .map(|reply| warp::reply::with_header(reply, "Sec-Websocket-Protocol", "graphql-ws"));

    let index = warp::fs::file("dist/index.html");

    let dir = warp::fs::dir("dist");

    // let routes = graphiql.or(graphql).or(notifications).or(dir).or(index);

    let routes = graphiql.or(graphql).or(dir).or(index);

    info!(
        root_logger.clone(),
        "Serving Mjolnir on {}:{}",
        addr.ip(),
        addr.socket()
    );
    warp::serve(routes).run(addr).await;

    Ok(())
}
