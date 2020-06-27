//use futures::Stream;
use juniper::{EmptySubscription, FieldResult, IntoFieldError, RootNode};
use slog::info;
use slog::Logger;
// use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqlitePool;

use super::indexes;

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
        let res = indexes::create_index(index, context)
            .await
            .map_err(IntoFieldError::into_field_error);
        info!(context.logger, "Done create index");
        res
    }
}

type Schema = RootNode<'static, Query, Mutation, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

// impl<S> juniper::FromInputValue<S> for indexes::IndexRequestBody
// where
//     S: juniper::ScalarValue,
// {
//     fn from_input_value<'a>(v: &juniper::InputValue<S>) -> Option<indexes::IndexRequestBody> {
//         let obj = match v.to_object_value() {
//             Some(o) => o,
//             None => return None,
//         };
//
//         Some(indexes::IndexRequestBody {
//             index_type: match obj.get("indexType").and_then(|v| v.convert()) {
//                 Some(f) => f,
//                 None => return None,
//             },
//             data_source: match obj.get("dataSource").and_then(|v| v.convert()) {
//                 Some(f) => f,
//                 None => return None,
//             },
//             region: match obj.get("region").and_then(|v| v.convert()) {
//                 Some(f) => f,
//                 None => return None,
//             },
//         })
//     }
// }
