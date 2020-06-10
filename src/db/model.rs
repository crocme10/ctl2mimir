use async_trait::async_trait;
use chrono::{DateTime, Utc};
use snafu::Snafu;
use std::convert::TryFrom;

pub type EntityId = i32;

pub struct IndexEntity {
    pub index_id: EntityId,
    pub index_type: String,
    pub data_source: String,
    pub regions: Vec<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ProvideData {
    async fn create_index(
        &mut self,
        index_type: &str,
        data_source: &str,
        // FIXME I'm not sure about this.... should it be &str, or &[&str]
        // More generally, should this interface be more typed??
        // The example I am using is based on REST interface, which
        // is not typed.... but for GraphQL, it could be different
        regions: &str,
    ) -> ProvideResult<IndexEntity>;
}

pub type ProvideResult<T> = Result<T, ProvideError>;

/// An error returned by a provider
#[derive(Debug, Snafu)]
pub enum ProvideError {
    /// The requested entity does not exist
    #[snafu(display("Entity does not exist"))]
    #[snafu(visibility(pub))]
    NotFound,

    /// The operation violates a uniqueness constraint
    #[snafu(display("Operation violates uniqueness constraint: {}", details))]
    #[snafu(visibility(pub))]
    UniqueViolation { details: String },

    /// The requested operation violates the data model
    #[snafu(display("Operation violates model: {} ({})", details, source))]
    #[snafu(visibility(pub))]
    ModelViolation {
        details: String,
        source: sqlx::Error,
    },

    /// The requested operation violates the data model
    #[snafu(display("UnHandled Error: {}", source))]
    #[snafu(visibility(pub))]
    UnHandledError { source: sqlx::Error },
}

impl From<sqlx::Error> for ProvideError {
    /// Convert a SQLx error into a provider error
    ///
    /// For Database errors we attempt to downcast
    ///
    /// FIXME(RFC): I have no idea if this is sane
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ProvideError::NotFound,

            sqlx::Error::Database(db_err) => {
                if let Some(sqlite_err) = db_err.try_downcast_ref::<sqlx::sqlite::SqliteError>() {
                    if let Ok(provide_err) = ProvideError::try_from(sqlite_err) {
                        return provide_err;
                    }
                }

                ProvideError::UnHandledError {
                    source: sqlx::Error::Database(db_err),
                }
            }
            _ => ProvideError::UnHandledError { source: err },
        }
    }
}
