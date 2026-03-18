use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

pub mod repo_assets;
pub mod repo_events;
pub mod repo_tag_generator;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial.sql",
        include_str!("../../migrations/0001_initial.sql"),
    ),
    (
        "0002_event_versioning.sql",
        include_str!("../../migrations/0002_event_versioning.sql"),
    ),
    (
        "0003_http_lifecycle_columns.sql",
        include_str!("../../migrations/0003_http_lifecycle_columns.sql"),
    ),
];

fn expected_schema_version() -> i32 {
    MIGRATIONS.len() as i32
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("incompatible schema version: found {found}, expected {expected}")]
    IncompatibleSchemaVersion { found: i32, expected: i32 },
}

pub fn open_and_prepare(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    let expected_version = expected_schema_version();
    let version = schema_version(&conn)?;
    if version > expected_version {
        return Err(DbError::IncompatibleSchemaVersion {
            found: version,
            expected: expected_version,
        });
    }

    if version < expected_version {
        apply_migrations(&conn, version)?;
    }

    Ok(conn)
}

fn apply_migrations(conn: &Connection, from_version: i32) -> Result<(), DbError> {
    for (idx, (_name, sql)) in MIGRATIONS.iter().enumerate() {
        let target_version = (idx as i32) + 1;
        if target_version <= from_version {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", target_version.to_string())?;
        tx.commit()?;
    }
    Ok(())
}

fn schema_version(conn: &Connection) -> Result<i32, DbError> {
    let value: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(value)
}
