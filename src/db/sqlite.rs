use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use sqlx::error::DatabaseError;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqliteError, SqliteQueryAs};
use sqlx::{Cursor, Executor, FromRow, SqliteConnection, SqlitePool};
use std::convert::TryFrom;

use crate::db::model::*;
use crate::db::Db;

impl TryFrom<&SqliteError> for ProvideError {
    type Error = ();

    /// Attempt to convert a Sqlite into a more-specific provider error
    ///
    /// Unexpected cases will be bounced back to the caller for handling
    ///
    /// * [Sqlite Error Codes](https://www.sqlite.org/rescode.html)
    fn try_from(db_err: &SqliteError) -> Result<Self, Self::Error> {
        let provider_err = match db_err.code().unwrap() {
            "2067" => ProvideError::UniqueViolation {
                details: db_err.message().to_owned(),
                // FIXME Can't find a way to add a source
                // source: err.into(),
                // sqlx::error::Error::Database(),
                //sqlx::error::Error::Database(Box::new(db_err)),
            },
            _ => return Err(()),
        };

        Ok(provider_err)
    }
}

#[derive(sqlx::FromRow)]
struct SqliteIndexEntity {
    index_id: EntityId,
    index_type: String,
    data_source: String,
    region: String,
    status: String,
    created_at: i32,
    updated_at: i32,
}

impl From<SqliteIndexEntity> for IndexEntity {
    fn from(entity: SqliteIndexEntity) -> Self {
        let SqliteIndexEntity {
            index_id,
            index_type,
            data_source,
            region,
            status,
            created_at,
            updated_at,
        } = entity;

        IndexEntity {
            index_id,
            index_type,
            data_source,
            region,
            status,
            created_at: Utc.timestamp(created_at as _, 0),
            updated_at: Utc.timestamp(updated_at as _, 0),
        }
    }
}

pub async fn connect(db_url: &str) -> sqlx::Result<SqlitePool> {
    let pool = SqlitePool::new(db_url).await?;
    Ok(pool)
}

#[async_trait]
impl Db for SqlitePool {
    type Conn = PoolConnection<SqliteConnection>;

    async fn conn(&self) -> sqlx::Result<Self::Conn> {
        self.acquire().await
    }
}

#[async_trait]
impl ProvideData for SqliteConnection {
    async fn create_index(
        &mut self,
        index_type: &str,
        data_source: &str,
        region: &str,
    ) -> ProvideResult<IndexEntity> {
        let rec: SqliteIndexEntity = sqlx::query_as(
            r#"
INSERT INTO indexes ( index_type, data_source, region )
VALUES ( $1, $2, $3 );
SELECT * FROM indexes WHERE index_id = last_insert_rowid();
            "#,
        )
        .bind(index_type)
        .bind(data_source)
        .bind(region)
        .fetch_one(self)
        .await?;

        Ok(rec.into())
    }

    async fn update_index_status(
        &mut self,
        index_id: EntityId,
        status: &str,
    ) -> ProvideResult<IndexEntity> {
        println!("About to update sqlite status {}", status);

        self.execute("SAVEPOINT update_index_status").await?;

        // 1
        let select_stmt = sqlx::query(
            r#"
SELECT * FROM indexes WHERE index_id = $1
            "#,
        )
        .bind(index_id);

        let rec = self
            .fetch(select_stmt)
            .next()
            .await?
            .map(|row| SqliteIndexEntity::from_row(&row).expect("invalid entity"))
            .expect("Cursor");

        println!("Precheck => {} {}", rec.status, rec.updated_at);

        // 2
        let update_stmt = sqlx::query(
            r#"
UPDATE indexes
SET status = $2, updated_at = (STRFTIME('%s', 'now'))
WHERE index_id = $1
            "#,
        )
        .bind(index_id)
        .bind(status);

        self.execute(update_stmt).await?;

        // 3
        let select_stmt = sqlx::query(
            r#"
SELECT * FROM indexes WHERE index_id = $1
            "#,
        )
        .bind(index_id);

        let rec = self
            .fetch(select_stmt)
            .next()
            .await?
            .map(|row| SqliteIndexEntity::from_row(&row).expect("invalid entity"))
            .expect("Cursor");

        println!("Postcheck => {} {}", rec.status, rec.updated_at);

        self.execute("RELEASE update_index_status").await?;

        println!("Done sqlite update => {}", rec.status);
        Ok(rec.into())
    }

    async fn get_all_indexes(&mut self) -> Result<Vec<IndexEntity>, ProvideError> {
        let recs: Vec<SqliteIndexEntity> = sqlx::query_as(
            r#"
            SELECT * FROM indexes ORDER BY updated_at
            "#,
        )
        .fetch_all(self)
        .await
        .map_err(|err| ProvideError::from(err))?;

        let entities = recs
            .into_iter()
            .map(|rec| IndexEntity::from(rec))
            .collect::<Vec<_>>();

        Ok(entities)
    }
}
