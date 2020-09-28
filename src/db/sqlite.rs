use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use slog::{debug, info, o, Logger};
use snafu::ResultExt;
use sqlx::error::DatabaseError;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqliteError, SqliteQueryAs};
use sqlx::{Cursor, Executor, FromRow, SqliteConnection, SqlitePool};
use std::convert::TryFrom;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::model::*;
use super::Db;
use crate::error;

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
        self.execute("SAVEPOINT update_index_status").await?;

        let update_stmt = sqlx::query(
            r#"
UPDATE indexes
SET status = $1, updated_at = (STRFTIME('%s', 'now'))
WHERE index_id = $2
            "#,
        )
        .bind(status)
        .bind(index_id);

        self.execute(update_stmt).await?;
        // let count = self.execute(update_stmt).await?;
        // println!("Count: {}", count);

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

        self.execute("RELEASE update_index_status").await?;

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

pub async fn init_db(conn_str: &str, logger: Logger) -> Result<(), error::Error> {
    info!(logger, "Initializing  DB @ {}", conn_str);
    migration_down(conn_str, &logger).await?;
    migration_up(conn_str, &logger).await?;
    Ok(())
}

pub async fn migration_up(conn_str: &str, logger: &Logger) -> Result<(), error::Error> {
    let clogger = logger.new(o!("database" => String::from(conn_str)));
    debug!(clogger, "Movine Up");
    let mut cmd = Command::new("movine");
    cmd.env("SQLITE_FILE", conn_str);
    cmd.arg("up");
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().context(error::TokioIOError {
        details: String::from("Failed to execute movine"),
    })?;

    let stdout = child.stdout.take().ok_or(error::Error::MiscError {
        details: String::from("child did not have a handle to stdout"),
    })?;

    let mut reader = BufReader::new(stdout).lines();

    // Ensure the child process is spawned in the runtime so it can
    // make progress on its own while we await for any output.
    tokio::spawn(async {
        // FIXME Need to do something about logging this and returning an error.
        let _status = child.await.expect("child process encountered an error");
        // println!("child status was: {}", status);
    });
    debug!(clogger, "Spawned migration up");

    while let Some(line) = reader.next_line().await.context(error::TokioIOError {
        details: String::from("Could not read from piped output"),
    })? {
        debug!(clogger, "movine: {}", line);
    }

    Ok(())
}

pub async fn migration_down(conn_str: &str, logger: &Logger) -> Result<(), error::Error> {
    let clogger = logger.new(o!("database" => String::from(conn_str)));
    debug!(clogger, "Movine Down");
    let mut cmd = Command::new("movine");
    cmd.env("SQLITE_FILE", conn_str);
    cmd.arg("down");
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().context(error::TokioIOError {
        details: String::from("Failed to execute movine"),
    })?;

    let stdout = child.stdout.take().ok_or(error::Error::MiscError {
        details: String::from("child did not have a handle to stdout"),
    })?;

    let mut reader = BufReader::new(stdout).lines();

    // Ensure the child process is spawned in the runtime so it can
    // make progress on its own while we await for any output.
    tokio::spawn(async {
        // FIXME Need to do something about logging this and returning an error.
        let _status = child.await.expect("child process encountered an error");
        // println!("child status was: {}", status);
    });
    debug!(clogger, "Spawned migration down");

    while let Some(line) = reader.next_line().await.context(error::TokioIOError {
        details: String::from("Could not read from piped output"),
    })? {
        debug!(clogger, "movine: {}", line);
    }

    Ok(())
}
