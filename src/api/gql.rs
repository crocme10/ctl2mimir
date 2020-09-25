use futures::Stream;
use juniper::{FieldError, FieldResult, IntoFieldError, RootNode};
use slog::Logger;
use slog::{debug, info};
use snafu::ResultExt;
use sqlx::sqlite::SqlitePool;
use std::env;
use std::pin::Pin;

use super::indexes;
use crate::error;
use crate::fsm::State;

// FIXME. The context should be generic in the type of the pool. But the macro derive
// juniper::graphql_object doesn't support yet generic contexts.
// See https://github.com/davidpdrsn/juniper-from-schema/issues/101
#[derive(Debug, Clone)]
pub struct Context {
    pub pool: SqlitePool,
    pub logger: Logger,
}

impl juniper::Context for Context {}

pub struct Query;

#[juniper::graphql_object(
    Context = Context
)]
impl Query {
    /// Return a list of all features
    async fn indexes(&self, context: &Context) -> FieldResult<indexes::MultIndexesResponseBody> {
        indexes::list_indexes(context)
            .await
            .map_err(IntoFieldError::into_field_error)
    }
}

pub struct Mutation;

#[juniper::graphql_object(
    Context = Context
)]
impl Mutation {
    /// Create an index
    async fn create_index(
        &self,
        index: indexes::IndexRequestBody,
        context: &Context,
    ) -> FieldResult<indexes::IndexResponseBody> {
        info!(context.logger, "Calling create index");
        let key = "ES_CONN_STR";
        let es = env::var(key).context(error::EnvError {
            details: format!("Could not retrieve environment variable {}", key),
        })?;
        let res = indexes::create_index(index, es, context)
            .await
            .map_err(IntoFieldError::into_field_error);
        info!(context.logger, "Done create index");
        res
    }
}

type IndexStatusUpdateStream =
    Pin<Box<dyn Stream<Item = Result<indexes::IndexStatusUpdateBody, FieldError>> + Send>>;

pub struct Subscription;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    async fn notifications(context: &Context) -> IndexStatusUpdateStream {
        // Ready a subscription connection to receive notifications from the FSM
        let zmq = async_zmq::subscribe("tcp://127.0.0.1:5555")
            .context(error::ZMQSocketError {
                details: String::from("Could not subscribe on tcp://127.0.0.1:5555"),
            })?
            .connect()
            .context(error::ZMQError {
                details: String::from("Could not connect subscribe"),
            })?;

        zmq.set_subscribe("state")
            .context(error::ZMQSubscribeError {
                details: format!("Could not subscribe to '{}' topic", "state"),
            })?;

        info!(context.logger, "Subscribed to ZMQ Publications");

        let logger = context.logger.clone();
        let stream = zmq.map(move |msg| {
            let msg = msg.context(error::ZMQRecvError {
                details: String::from("ZMQ Reception Error"),
            })?;
            info!(logger, "Received something on GraphQL Subscription channel");

            // The msg we receive is made of three parts, the topic, the id, and the serialized status.
            // Here, we skip the topic, and extract the id.
            let id = msg
                .iter()
                .skip(1) // skip the topic
                .next()
                .ok_or(error::Error::MiscError {
                    details: String::from(
                        "Just one item in a multipart message. That is plain wrong!",
                    ),
                })?
                .as_str()
                .ok_or(error::Error::MiscError {
                    details: String::from("Status Message is not valid UTF8"),
                })?
                .parse::<i32>()
                .context(error::ParseIntError {
                    details: "Could not get id",
                })?;

            // The msg we receive is made of three parts, the topic, the id, and the serialized status.
            // Here, we skip the topic, and the id, and extract the status.
            let status = msg
                .iter()
                .skip(2)
                .next()
                .ok_or(error::Error::MiscError {
                    details: String::from(
                        "Just one item in a multipart message. That is plain wrong!",
                    ),
                })?
                .as_str()
                .ok_or(error::Error::MiscError {
                    details: String::from("Status Message is not valid UTF8"),
                })?;

            // info!(logger, "Received status {}", status);

            // The msg we have left should be a serialized version of the status.
            if let Err(err) = serde_json::from_str::<State>(status).context(error::SerdeJSONError {
                details: String::from("Could not deserialize state"),
            }) {
                info!(logger, "Deserialize error: {}", err);
            }

            // info!(logger, "Deserialized status {:?}", status);

            let resp = indexes::IndexStatusUpdateBody {
                id,
                status: String::from(status),
            };
            debug!(logger, "GraphQL Notification: {:?}", resp);
            Ok(resp)
        });

        Box::pin(stream)
    }
}

type Schema = RootNode<'static, Query, Mutation, Subscription>;

pub fn schema() -> Schema {
    Schema::new(Query, Mutation, Subscription)
}
