use futures::TryFutureExt;
use juniper::{GraphQLInputObject, GraphQLObject};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use sqlx::Connection;
use std::convert::TryFrom;

use crate::api::gql::Context;
use crate::api::model::*;
use crate::db::model::ProvideData;
use crate::db::Db;
use crate::error;

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

        let index = Index::from(entity);

        tx.commit().await.context(error::DBError {
            details: "could not commit transaction",
        })?;

        Ok(IndexResponseBody::from(IndexResponseBody { index }))
    }
    .await
}
