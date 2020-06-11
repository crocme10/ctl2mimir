use chrono::{DateTime, Utc};
use juniper::GraphQLObject;
use serde::Serialize;

use crate::db::model::*;

/// An index
#[derive(Debug, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub(in crate::api) struct Index {
    pub index_type: String,
    pub data_source: String,
    pub regions: Vec<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<IndexEntity> for Index {
    fn from(entity: IndexEntity) -> Self {
        let IndexEntity {
            index_type,
            data_source,
            regions,
            status,
            created_at,
            updated_at,
            ..
        } = entity;

        Index {
            index_type,
            data_source,
            regions,
            status,
            created_at,
            updated_at,
        }
    }
}
