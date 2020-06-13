//use chrono::{DateTime, Utc};
use futures::TryFutureExt;
// use heck::KebabCase;
// use log::*;
use juniper::GraphQLObject;
use serde::Serialize;
use snafu::ResultExt;
// use sqlx::pool::PoolConnection;
use sqlx::Connection;
use std::convert::TryFrom;

use crate::api::model::*;
// use crate::api::utils::*;
use crate::api::gql::Context;
use crate::db::model::ProvideData;
use crate::db::Db;
use crate::error;

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
///
/// [List Indexes](https://github.com/gothinkster/realworld/tree/master/api#list-indexes)
///   Request<impl Db<Conn = PoolConnection<impl Connect + ProvideData>>>,
// pub async fn list_indexes<S, C>(
//     context: &Context<S>,
// ) -> Result<MultIndexesResponseBody, error::Error>
// where
//     S: Send + Sync + Db<Conn = PoolConnection<C>> + 'static,
//     C: sqlx::Connect + ProvideData,
// {
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
