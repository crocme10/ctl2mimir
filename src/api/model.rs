use chrono::{DateTime, Utc};
use juniper::GraphQLObject;
use serde::Serialize;

use crate::db::model::*;

/// An index
// The status is a string, but it should be a state (as in FSM::State).
// But for this to work, I'd have to implement GraphQLEnum for FSM::State,
// and the enum would have to be anonymous.
#[derive(Debug, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub(in crate::api) struct Index {
    pub index_id: EntityId,
    pub index_type: String,
    pub data_source: String,
    pub region: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<IndexEntity> for Index {
    fn from(entity: IndexEntity) -> Self {
        let IndexEntity {
            index_id,
            index_type,
            data_source,
            region,
            status,
            created_at,
            updated_at,
            ..
        } = entity;

        Index {
            index_id,
            index_type,
            data_source,
            region,
            status,
            created_at,
            updated_at,
        }
    }
}
