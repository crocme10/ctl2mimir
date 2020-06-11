//use futures::Stream;
use juniper::{EmptyMutation, EmptySubscription, FieldResult, IntoFieldError, RootNode};
use slog::Logger;
use sqlx::pool::PoolConnection;

use super::{indexes, model};
use crate::db::{self, model::ProvideData, Db};

#[derive(Debug, Clone)]
pub struct Context<S> {
    pub pool: S,
    pub logger: Logger,
}

impl<S> juniper::Context for Context<S> {}

pub struct Query;

#[juniper::graphql_object(
    Context = Context<S>
)]
impl Query {
    /// Return a list of all features
    async fn indexes<S>(
        &self,
        context: &Context<S>,
    ) -> FieldResult<indexes::MultIndexesResponseBody>
    where
        S: Send + Sync + Db<Conn = PoolConnection<C>> + 'static,
        C: sqlx::Connect + ProvideData,
    {
        indexes::list_indexes(context)
            .await
            .map_err(IntoFieldError::into_field_error)
    }
}

type Schema = RootNode<'static, Query, EmptyMutation<u32>, EmptySubscription<u32>>;

pub fn schema() -> Schema {
    Schema::new(Query, EmptyMutation::new(), EmptySubscription::new())
}
