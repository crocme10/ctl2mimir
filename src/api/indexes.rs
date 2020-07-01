use async_zmq::StreamExt;
use futures::TryFutureExt;
use juniper::{GraphQLInputObject, GraphQLObject};
use serde::{Deserialize, Serialize};
use slog::{debug, info};
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
        let state = &context.pool;

        let mut tx = state
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                details: "could not retrieve indexes",
            })?;

        let entities = tx.get_all_indexes().await.context(error::DBProvideError {
            details: "Could not get all them indexes",
        })?;

        let indexes = entities
            .into_iter()
            .map(|ent| Index::from(ent))
            .collect::<Vec<_>>();

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
            context.logger,
            "Creating Index {} {} {}", index_type, data_source, region
        );

        let index = create_db(&context, &index_type, &data_source, &region).await?;
        let id = index.index_id;

        // Now construct and initialize the Finite State Machine (FSM)
        // state is the name of the topic we're asking the publisher to broadcast message,
        // 5555 is the port
        let fsm = fsm::FSM::new(
            id,
            index_type,
            data_source,
            region,
            String::from("state"),
            5555,
        )?;

        // Listen to FSM for updates
        let ct2 = context.clone();
        tokio::spawn(update_notifications(ct2, id));
        info!(context.logger, "Listening to state changes");

        tokio::spawn(fsm::exec(fsm));
        info!(context.logger, "Running FSM");

        Ok(IndexResponseBody::from(IndexResponseBody { index }))
    }
    .await
}

async fn update_notifications(context: Context, index_id: EntityId) -> Result<(), error::Error> {
    // Ready a subscription connection to receive notifications from the FSM
    let mut zmq = async_zmq::subscribe("tcp://127.0.0.1:5555")
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
    // and listen for notifications
    while let Some(msg) = zmq.next().await {
        // Received message is a type of Result<MessageBuf>
        let msg = msg.context(error::ZMQRecvError {
            details: String::from("ZMQ Reception Error"),
        })?;

        // The msg we receive is made of three parts, the topic, the id, and the serialized status.
        // Here, we skip the topic, and extract the second part.
        let msg = msg
            .iter()
            .skip(2) // skip the topic and the id // FIXME use the id
            .next()
            .ok_or(error::Error::MiscError {
                details: String::from("Just one item in a multipart message. That is plain wrong!"),
            })?
            .as_str()
            .ok_or(error::Error::MiscError {
                details: String::from("Status Message is not valid UTF8"),
            })?;

        debug!(logger, "API Received {}", msg);
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
    let state = &context.pool;

    let mut tx = state
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
    let state = &context.pool;
    let mut tx = state
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
