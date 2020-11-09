use async_zmq::StreamExt;
use futures::TryFutureExt;
use juniper::{GraphQLInputObject, GraphQLObject};
use serde::{Deserialize, Serialize};
use slog::info;
use snafu::ResultExt;
use sqlx::Connection;
use std::convert::TryFrom;

use crate::api::gql::Context;
use crate::api::model::*;
use crate::db::model::{EntityId, ProvideData};
use crate::db::Db;
use crate::error;
use crate::fsm;

/// The request body for a single index
#[derive(Debug, Serialize, Deserialize, GraphQLInputObject)]
pub struct IndexRequestBody {
    pub index_type: String,
    pub data_source: String,
    pub region: String,
}

/// The response body for a single index
#[derive(Debug, Serialize, GraphQLObject)]
pub struct IndexResponseBody {
    index: Index,
}

/// The response body for a stream of status update The status is untyped (its just a string),
/// because the State type is a rich enum type, which juniper does not support.
#[derive(Debug, Serialize, GraphQLObject)]
pub struct IndexStatusUpdateBody {
    pub id: EntityId,
    pub status: String,
}

/// The response body for multiple indexes
#[derive(Debug, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct MultIndexesResponseBody {
    indexes: Vec<Index>,
    indexes_count: i32,
}

impl From<Vec<Index>> for MultIndexesResponseBody {
    fn from(indexes: Vec<Index>) -> Self {
        let indexes_count = i32::try_from(indexes.len()).unwrap();
        Self {
            indexes,
            indexes_count,
        }
    }
}

/// Retrieve all indexes
pub async fn list_indexes(context: &Context) -> Result<MultIndexesResponseBody, error::Error> {
    async move {
        let pool = &context.state.pool;

        let mut tx = pool
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                details: "could not retrieve indexes",
            })?;

        let entities = tx.get_all_indexes().await.context(error::DBProvideError {
            details: "Could not get all them indexes",
        })?;

        let indexes = entities.into_iter().map(Index::from).collect::<Vec<_>>();

        tx.commit().await.context(error::DBError {
            details: "could not retrieve indexes",
        })?;

        Ok(MultIndexesResponseBody::from(indexes))
    }
    .await
}

/// Create a new index
pub async fn create_index(
    index_request: IndexRequestBody,
    context: &Context,
) -> Result<IndexResponseBody, error::Error> {
    async move {
        let IndexRequestBody {
            index_type,
            data_source,
            region,
        } = index_request;

        info!(
            context.state.logger,
            "Creating Index {} {} {}", index_type, data_source, region
        );

        let index = create_db(&context, &index_type, &data_source, &region).await?;
        let id = index.index_id;

        let fsm = fsm::FSM::new(
            id,
            index_type,
            data_source,
            region,
            &context.state.settings,
            String::from("state"),
            context.state.logger.clone(),
        )?;

        // Listen to FSM for updates
        let ct2 = context.clone();
        tokio::spawn(update_notifications(ct2, id));
        info!(context.state.logger, "Listening to state changes");

        tokio::spawn(fsm::exec(fsm));
        info!(context.state.logger, "Running FSM");

        Ok(IndexResponseBody { index })
    }
    .await
}

async fn update_notifications(context: Context, index_id: EntityId) -> Result<(), error::Error> {
    // Ready a subscription connection to receive notifications from the FSM
    let zmq_endpoint = format!(
        "tcp://{}:{}",
        context.state.settings.zmq.host, context.state.settings.zmq.port
    );
    let zmq_topic = &context.state.settings.zmq.topic;
    let mut zmq = async_zmq::subscribe(&zmq_endpoint)
        .context(error::ZMQSocketError {
            details: format!("Could not subscribe to zmq endpoint at {}", &zmq_endpoint),
        })?
        .connect()
        .context(error::ZMQError {
            details: String::from("Could not connect subscribe"),
        })?;

    zmq.set_subscribe(&zmq_topic)
        .context(error::ZMQSubscribeError {
            details: format!("Could not subscribe to '{}' topic", &zmq_topic),
        })?;

    info!(
        context.state.logger,
        "Subscribed to ZMQ Publications on endpoint {} / topic {}", &zmq_endpoint, &zmq_topic
    );

    let logger = context.state.logger.clone();
    // and listen for notifications
    while let Some(msg) = zmq.next().await {
        // Received message is a type of Result<MessageBuf>
        let msg = msg.context(error::ZMQRecvError {
            details: String::from("ZMQ Reception Error"),
        })?;

        // The msg we receive is made of three parts, the topic, the id, and the serialized status.
        // Here, we skip the topic, and extract the second part.
        let msg = msg
            .get(2) // skip the topic and the id // FIXME use the id
            .ok_or(error::Error::MiscError {
                details: String::from("Just one item in a multipart message. That is plain wrong!"),
            })?
            .as_str()
            .ok_or(error::Error::MiscError {
                details: String::from("Status Message is not valid UTF8"),
            })?;

        info!(logger, "API Received {}", msg);
        // The msg we have left should be a serialized version of the status.
        let status = serde_json::from_str(msg).context(error::SerdeJSONError {
            details: String::from("Could not deserialize state"),
        })?;

        update_db(&context, index_id, msg).await?;

        match status {
            fsm::State::NotAvailable => {
                break;
            }
            fsm::State::Available => {
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn update_db(
    context: &Context,
    index_id: EntityId,
    msg: &str,
) -> Result<Index, error::Error> {
    // We now have a valid status, so we proceed with updating the database.
    let pool = &context.state.pool;

    let mut tx = pool
        .conn()
        .and_then(Connection::begin)
        .await
        .context(error::DBError {
            details: "could not retrieve transaction",
        })?;

    let entity = tx
        .update_index_status(index_id, msg)
        .await
        .context(error::DBProvideError {
            details: "Could not update index status",
        })?;

    tx.commit().await.context(error::DBError {
        details: "could not commit transaction",
    })?;

    Ok(Index::from(entity))
}

async fn create_db(
    context: &Context,
    index_type: &str,
    data_source: &str,
    region: &str,
) -> Result<Index, error::Error> {
    let pool = &context.state.pool;
    let mut tx = pool
        .conn()
        .and_then(Connection::begin)
        .await
        .context(error::DBError {
            details: "could not retrieve transaction",
        })?;

    let entity = tx
        .create_index(&index_type, &data_source, &region)
        .await
        .context(error::DBProvideError {
            details: "Could not create index",
        })?;

    tx.commit().await.context(error::DBError {
        details: "could not commit transaction",
    })?;

    Ok(Index::from(entity))
}
