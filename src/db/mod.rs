use async_trait::async_trait;

/// Database implementation for SQLite
pub mod sqlite;

/// Database models
pub mod model;

/// A type that abstracts a database
#[async_trait]
pub trait Db {
    type Conn;

    async fn conn(&self) -> sqlx::Result<Self::Conn>;
}
