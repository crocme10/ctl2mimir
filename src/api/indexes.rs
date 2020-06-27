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
///
/// [API Spec](https://github.com/gothinkster/realworld/tree/master/api#single-index)
#[derive(Debug, Serialize, GraphQLObject)]
pub struct IndexResponseBody {
    index: Index,
}

/// The response body for multiple indexes
///
/// [API Spec](https://github.com/gothinkster/realworld/tree/master/api#multiple-comments)
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
        let state = &context.pool;

        info!(
            context.logger,
            "Creating Index {} {} {}",
            index_request.index_type,
            index_request.data_source,
            index_request.region
        );

        let mut tx = state
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                details: "could not retrieve transaction",
            })?;

        let IndexRequestBody {
            index_type,
            data_source,
            region,
        } = index_request;

        let entity = tx
            .create_index(&index_type, &data_source, &region)
            .await
            .context(error::DBProvideError {
                details: "Could not create index",
            })?;

        let id = entity.index_id; // We save the id for updating the status later.
        let index = Index::from(entity);

        tx.commit().await.context(error::DBError {
            details: "could not commit transaction",
        })?;

        info!(context.logger, "Index initialized in Sqlite");

        // Now construct and initialize the Finite State Machine (FSM)
        // state is the name of the topic we're asking the publisher to broadcast message,
        // 5555 is the port
        let fsm = fsm::FSM::new(index_type, data_source, region, String::from("state"), 5555)?;

        // Listen to FSM for updates
        let ct2 = context.clone();
        tokio::spawn(update_notifications(ct2, id));

        // Start the FSM
        tokio::spawn(fsm::exec(fsm));

        // let _ = task_exec.await.context(error::TokioJoinError {
        //     details: String::from("Could not run FSM to completion"),
        // })?;

        // let _ = task_update.await.context(error::TokioJoinError {
        //     details: String::from("Could not update FSM to completion"),
        // })?;

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

    // and listen for notifications
    while let Some(msg) = zmq.next().await {
        // Received message is a type of Result<MessageBuf>
        let msg = msg.context(error::ZMQRecvError {
            details: String::from("ZMQ Reception Error"),
        })?;

        // The msg we receive is made of two parts, the topic, and the serialized status.
        // Here, we skip the topic, and extract the second part.
        let msg = msg
            .iter()
            .skip(1) // skip the topic
            .next()
            .ok_or(error::Error::MiscError {
                details: String::from("Just one item in a multipart message. That is plain wrong!"),
            })?
            .as_str()
            .ok_or(error::Error::MiscError {
                details: String::from("Status Message is not valid UTF8"),
            })?;

        info!(context.logger, "Received: {}", msg);

        // The msg we have left should be a serialized version of the status.
        let status = serde_json::from_str(msg).context(error::SerdeJSONError {
            details: String::from("Could not deserialize state"),
        })?;

        // We now have a valid status, so we proceed with updating the database.
        let state = &context.pool;

        let mut tx = state
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                details: "could not retrieve transaction",
            })?;

        info!(
            context.logger,
            "Updating index {} with status {}", index_id, msg
        );

        let entity =
            tx.update_index_status(index_id, msg)
                .await
                .context(error::DBProvideError {
                    details: "Could not update index status",
                })?;

        info!(
            context.logger,
            "Index update successful {} => {}", entity.index_id, entity.status
        );

        tx.commit().await.context(error::DBError {
            details: "could not commit transaction",
        })?;

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
