use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use domain::{
    apply_event, DomainState, Event, EventApplyError, EventType, FieldType, MutationSpec,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct EventMutationRow {
    pub mutation_index: i64,
    pub operation: String,
    pub tag_definition_id: i64,
    pub input_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EventTypeVersionRecord {
    pub event_type_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub version: i64,
    pub mutations: Vec<EventMutationRow>,
}

#[derive(Debug, Error)]
pub enum EventTypeCreateError {
    #[error("event type already exists")]
    AlreadyExists,
    #[error("tag definition not found: {0}")]
    TagDefinitionMissing(i64),
    #[error("unsupported mutation operation: {0}")]
    UnsupportedMutationOperation(String),
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum EventTypeVersionCreateError {
    #[error("event type not found")]
    EventTypeNotFound,
    #[error("tag definition not found: {0}")]
    TagDefinitionMissing(i64),
    #[error("unsupported mutation operation: {0}")]
    UnsupportedMutationOperation(String),
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum EventTypeDeleteVersionError {
    #[error("event type not found")]
    EventTypeNotFound,
    #[error("event type version not found")]
    VersionNotFound,
    #[error("event type version in use")]
    VersionInUse,
    #[error("cannot delete only version")]
    CannotDeleteOnlyVersion,
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum EventApplyRepoError {
    #[error("asset not found")]
    AssetNotFound,
    #[error("event type not found")]
    EventTypeNotFound,
    #[error("idempotency key payload mismatch")]
    IdempotencyPayloadMismatch,
    #[error("event type mutation operation invalid: {0}")]
    EventTypeMutationInvalid(String),
    #[error("enum option not found for `{field_id}`: {option_key}")]
    EnumOptionNotFound {
        field_id: String,
        option_key: String,
    },
    #[error("external entity not found for `{field_id}`: {entity_id}")]
    ExternalEntityNotFound { field_id: String, entity_id: i64 },
    #[error(
        "external entity type mismatch for `{field_id}`: entity {entity_id} has type {found_type_id}, expected {expected_type_id}"
    )]
    ExternalEntityTypeMismatch {
        field_id: String,
        entity_id: i64,
        expected_type_id: i64,
        found_type_id: i64,
    },
    #[error("external entity type is not configured for `{0}`")]
    ExternalEntityTypeMissing(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct AppliedEventRecord {
    pub event_id: i64,
    pub asset_id: i64,
    pub event_type_id: String,
    pub event_type_version: i64,
    pub payload: Value,
    pub created_at: String,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct EventListItem {
    pub event_id: i64,
    pub event_type_id: String,
    pub event_type_version: i64,
    pub payload: Value,
    pub timestamp: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct EventListPage {
    pub items: Vec<EventListItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Error)]
pub enum EventListError {
    #[error("asset not found")]
    AssetNotFound,
    #[error("invalid cursor")]
    InvalidCursor,
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct NewEventMutation {
    pub mutation_index: i64,
    pub operation: String,
    pub tag_definition_id: i64,
    pub input_key: Option<String>,
}

pub fn create_event_type_initial_version(
    conn: &mut Connection,
    event_type_id: &str,
    display_name: &str,
    description: Option<&str>,
    mutations: &[NewEventMutation],
) -> Result<EventTypeVersionRecord, EventTypeCreateError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    for mutation in mutations {
        if !is_supported_operation(&mutation.operation) {
            return Err(EventTypeCreateError::UnsupportedMutationOperation(
                mutation.operation.clone(),
            ));
        }
    }

    for mutation in mutations {
        let exists = tx
            .query_row(
                "SELECT 1 FROM tag_definitions WHERE id = ?1",
                [mutation.tag_definition_id],
                |_row| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(EventTypeCreateError::TagDefinitionMissing(
                mutation.tag_definition_id,
            ));
        }
    }

    let insert = tx.execute(
        "INSERT INTO event_types (event_type_id, display_name, description, current_version) VALUES (?1, ?2, ?3, 1)",
        params![event_type_id, display_name, description],
    );
    if let Err(err) = insert {
        if format!("{err}").contains("UNIQUE constraint failed: event_types.event_type_id") {
            return Err(EventTypeCreateError::AlreadyExists);
        }
        return Err(EventTypeCreateError::Sql(err));
    }

    insert_mutations(&tx, event_type_id, 1, mutations)?;
    tx.commit()?;

    load_event_type_version(conn, event_type_id, 1)?.ok_or(EventTypeCreateError::Sql(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn create_event_type_version(
    conn: &mut Connection,
    event_type_id: &str,
    mutations: &[NewEventMutation],
) -> Result<EventTypeVersionRecord, EventTypeVersionCreateError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    for mutation in mutations {
        if !is_supported_operation(&mutation.operation) {
            return Err(EventTypeVersionCreateError::UnsupportedMutationOperation(
                mutation.operation.clone(),
            ));
        }
    }

    let current_version: i64 = tx
        .query_row(
            "SELECT current_version FROM event_types WHERE event_type_id = ?1",
            [event_type_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(EventTypeVersionCreateError::EventTypeNotFound)?;

    for mutation in mutations {
        let exists = tx
            .query_row(
                "SELECT 1 FROM tag_definitions WHERE id = ?1",
                [mutation.tag_definition_id],
                |_row| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(EventTypeVersionCreateError::TagDefinitionMissing(
                mutation.tag_definition_id,
            ));
        }
    }

    let new_version = current_version + 1;
    insert_mutations(&tx, event_type_id, new_version, mutations)?;
    tx.execute(
        "UPDATE event_types SET current_version = ?1 WHERE event_type_id = ?2",
        params![new_version, event_type_id],
    )?;
    tx.commit()?;

    load_event_type_version(conn, event_type_id, new_version)?.ok_or(
        EventTypeVersionCreateError::Sql(rusqlite::Error::QueryReturnedNoRows),
    )
}

pub fn load_event_type_current(
    conn: &Connection,
    event_type_id: &str,
) -> Result<Option<EventTypeVersionRecord>, rusqlite::Error> {
    let current = conn
        .query_row(
            "SELECT current_version FROM event_types WHERE event_type_id = ?1",
            [event_type_id],
            |row| row.get(0),
        )
        .optional()?;

    match current {
        Some(version) => load_event_type_version(conn, event_type_id, version),
        None => Ok(None),
    }
}

pub fn load_event_type_version(
    conn: &Connection,
    event_type_id: &str,
    version: i64,
) -> Result<Option<EventTypeVersionRecord>, rusqlite::Error> {
    let head = conn
        .query_row(
            "SELECT event_type_id, display_name, description FROM event_types WHERE event_type_id = ?1",
            [event_type_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;

    let Some((event_type_id, display_name, description)) = head else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "
        SELECT mutation_index, operation, tag_definition_id, input_key
        FROM event_type_mutations
        WHERE event_type_id = ?1 AND event_type_version = ?2
        ORDER BY mutation_index
        ",
    )?;
    let mut rows = stmt.query(params![event_type_id, version])?;
    let mut mutations = Vec::new();
    while let Some(row) = rows.next()? {
        mutations.push(EventMutationRow {
            mutation_index: row.get(0)?,
            operation: row.get(1)?,
            tag_definition_id: row.get(2)?,
            input_key: row.get(3)?,
        });
    }

    let exists = conn
        .query_row(
            "SELECT 1 FROM event_type_mutations WHERE event_type_id = ?1 AND event_type_version = ?2 LIMIT 1",
            params![event_type_id, version],
            |_row| Ok(()),
        )
        .optional()?
        .is_some();

    if !exists {
        return Ok(None);
    }

    Ok(Some(EventTypeVersionRecord {
        event_type_id,
        display_name,
        description,
        version,
        mutations,
    }))
}

pub fn delete_event_type_version(
    conn: &mut Connection,
    event_type_id: &str,
    version: i64,
) -> Result<(), EventTypeDeleteVersionError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let current_version: i64 = tx
        .query_row(
            "SELECT current_version FROM event_types WHERE event_type_id = ?1",
            [event_type_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(EventTypeDeleteVersionError::EventTypeNotFound)?;

    let exists = tx
        .query_row(
            "SELECT 1 FROM event_type_mutations WHERE event_type_id = ?1 AND event_type_version = ?2 LIMIT 1",
            params![event_type_id, version],
            |_row| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists {
        return Err(EventTypeDeleteVersionError::VersionNotFound);
    }

    let in_use = tx
        .query_row(
            "SELECT 1 FROM asset_events WHERE event_type_id = ?1 AND event_type_version = ?2 LIMIT 1",
            params![event_type_id, version],
            |_row| Ok(()),
        )
        .optional()?
        .is_some();
    if in_use {
        return Err(EventTypeDeleteVersionError::VersionInUse);
    }

    let versions_count: i64 = tx.query_row(
        "SELECT COUNT(DISTINCT event_type_version) FROM event_type_mutations WHERE event_type_id = ?1",
        [event_type_id],
        |row| row.get(0),
    )?;
    if versions_count <= 1 {
        return Err(EventTypeDeleteVersionError::CannotDeleteOnlyVersion);
    }

    tx.execute(
        "DELETE FROM event_type_mutations WHERE event_type_id = ?1 AND event_type_version = ?2",
        params![event_type_id, version],
    )?;

    if version == current_version {
        let new_current: i64 = tx.query_row(
            "SELECT MAX(event_type_version) FROM event_type_mutations WHERE event_type_id = ?1",
            [event_type_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE event_types SET current_version = ?1 WHERE event_type_id = ?2",
            params![new_current, event_type_id],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn apply_asset_event(
    conn: &mut Connection,
    asset_tag: &str,
    idempotency_key: &str,
    event_type_id: &str,
    payload: Value,
) -> Result<AppliedEventRecord, EventApplyRepoError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let asset_id: i64 = tx
        .query_row(
            "SELECT id FROM assets WHERE asset_tag = ?1 AND deleted_at IS NULL",
            [asset_tag],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(EventApplyRepoError::AssetNotFound)?;

    let canonical_payload = canonicalize_json_value(&payload);
    let canonical_payload_str = serde_json::to_string(&canonical_payload)
        .map_err(|e| EventApplyRepoError::InvalidPayload(e.to_string()))?;
    let scope_hash = idempotency_scope_hash(
        "POST",
        &format!("/assets/{asset_tag}/events"),
        asset_id,
        &canonical_payload_str,
    );

    if let Some(existing) = tx
        .query_row(
            "
            SELECT id, asset_id, event_type_id, event_type_version, payload_json, created_at
            FROM asset_events
            WHERE asset_id = ?1 AND idempotency_key = ?2
            ",
            params![asset_id, idempotency_key],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
    {
        let existing_payload: Value = serde_json::from_str(&existing.4)
            .map_err(|e| EventApplyRepoError::InvalidPayload(e.to_string()))?;
        let existing_payload_str =
            serde_json::to_string(&canonicalize_json_value(&existing_payload))
                .map_err(|e| EventApplyRepoError::InvalidPayload(e.to_string()))?;
        let existing_hash = idempotency_scope_hash(
            "POST",
            &format!("/assets/{asset_tag}/events"),
            asset_id,
            &existing_payload_str,
        );

        if existing_hash != scope_hash || existing.2 != event_type_id {
            return Err(EventApplyRepoError::IdempotencyPayloadMismatch);
        }

        tx.commit()?;
        return Ok(AppliedEventRecord {
            event_id: existing.0,
            asset_id: existing.1,
            event_type_id: existing.2,
            event_type_version: existing.3,
            payload: existing_payload,
            created_at: existing.5,
            replayed: true,
        });
    }

    let event_type = load_event_type_for_apply(&tx, event_type_id)?
        .ok_or(EventApplyRepoError::EventTypeNotFound)?;

    let mut state = load_asset_state(&tx, asset_id)?;
    let domain_event = Event {
        idempotency_key: idempotency_key.to_string(),
        event_type_id: event_type.event_type_id.clone(),
        event_type_version: event_type.event_type_version,
        payload: json_object(payload)?,
    };

    apply_event(&mut state, &event_type, &domain_event)
        .map_err(|e| EventApplyRepoError::InvalidPayload(event_apply_message(&e)))?;

    tx.execute(
        "INSERT INTO asset_events (asset_id, idempotency_key, event_type_id, event_type_version, payload_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            asset_id,
            idempotency_key,
            event_type_id,
            i64::from(event_type.event_type_version),
            canonical_payload_str
        ],
    )?;
    let event_id = tx.last_insert_rowid();

    persist_asset_state(&tx, asset_id, &state)?;

    let created_at: String = tx.query_row(
        "SELECT created_at FROM asset_events WHERE id = ?1",
        [event_id],
        |row| row.get(0),
    )?;

    tx.commit()?;

    Ok(AppliedEventRecord {
        event_id,
        asset_id,
        event_type_id: event_type_id.to_string(),
        event_type_version: i64::from(event_type.event_type_version),
        payload: canonical_payload,
        created_at,
        replayed: false,
    })
}

pub fn list_asset_events(
    conn: &Connection,
    asset_tag: &str,
    limit: usize,
    cursor: Option<&str>,
) -> Result<EventListPage, EventListError> {
    let asset_id: i64 = conn
        .query_row(
            "SELECT id FROM assets WHERE asset_tag = ?1 AND deleted_at IS NULL",
            [asset_tag],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(EventListError::AssetNotFound)?;

    let (cursor_timestamp, cursor_event_id) = match cursor {
        Some(raw) => {
            let cursor_event_id = decode_cursor(raw).ok_or(EventListError::InvalidCursor)?;
            let cursor_timestamp: String = conn
                .query_row(
                    "SELECT created_at FROM asset_events WHERE asset_id = ?1 AND id = ?2",
                    params![asset_id, cursor_event_id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or(EventListError::InvalidCursor)?;
            (Some(cursor_timestamp), Some(cursor_event_id))
        }
        None => (None, None),
    };

    let mut items = Vec::new();
    if let (Some(ts), Some(eid)) = (cursor_timestamp.clone(), cursor_event_id) {
        let mut stmt = conn.prepare(
            "
            SELECT id, event_type_id, event_type_version, payload_json, created_at, idempotency_key
            FROM asset_events
            WHERE asset_id = ?1 AND (created_at < ?2 OR (created_at = ?2 AND id < ?3))
            ORDER BY created_at DESC, id DESC
            LIMIT ?4
            ",
        )?;
        let mut rows = stmt.query(params![asset_id, ts, eid, (limit + 1) as i64])?;
        while let Some(row) = rows.next()? {
            let payload_str: String = row.get(3)?;
            let payload: Value =
                serde_json::from_str(&payload_str).unwrap_or(Value::Object(Map::new()));
            items.push(EventListItem {
                event_id: row.get(0)?,
                event_type_id: row.get(1)?,
                event_type_version: row.get(2)?,
                payload,
                timestamp: row.get(4)?,
                idempotency_key: row.get(5)?,
            });
        }
    } else {
        let mut stmt = conn.prepare(
            "
            SELECT id, event_type_id, event_type_version, payload_json, created_at, idempotency_key
            FROM asset_events
            WHERE asset_id = ?1
            ORDER BY created_at DESC, id DESC
            LIMIT ?2
            ",
        )?;
        let mut rows = stmt.query(params![asset_id, (limit + 1) as i64])?;
        while let Some(row) = rows.next()? {
            let payload_str: String = row.get(3)?;
            let payload: Value =
                serde_json::from_str(&payload_str).unwrap_or(Value::Object(Map::new()));
            items.push(EventListItem {
                event_id: row.get(0)?,
                event_type_id: row.get(1)?,
                event_type_version: row.get(2)?,
                payload,
                timestamp: row.get(4)?,
                idempotency_key: row.get(5)?,
            });
        }
    }

    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }

    let next_cursor = if has_more {
        items
            .last()
            .map(|last| encode_cursor(&last.timestamp, last.event_id))
    } else {
        None
    };

    Ok(EventListPage { items, next_cursor })
}

fn insert_mutations(
    conn: &Connection,
    event_type_id: &str,
    version: i64,
    mutations: &[NewEventMutation],
) -> Result<(), rusqlite::Error> {
    for mutation in mutations {
        conn.execute(
            "
            INSERT INTO event_type_mutations (
                event_type_id,
                event_type_version,
                mutation_index,
                operation,
                tag_definition_id,
                input_key
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                event_type_id,
                version,
                mutation.mutation_index,
                mutation.operation,
                mutation.tag_definition_id,
                mutation.input_key
            ],
        )?;
    }
    Ok(())
}

fn parse_field_type(value_type: &str, external_entity_type_id: Option<i64>) -> FieldType {
    match value_type {
        "text" => FieldType::Text,
        "integer" => FieldType::Integer,
        "decimal" => FieldType::Decimal,
        "boolean" => FieldType::Boolean,
        "date" => FieldType::Date,
        "datetime" => FieldType::Datetime,
        "money" => FieldType::Money,
        "url" => FieldType::Url,
        "mac_address" => FieldType::MacAddress,
        "ipv4" => FieldType::Ipv4,
        "enum" => FieldType::Enum,
        "external_entity" => FieldType::ExternalEntity(external_entity_type_id.unwrap_or(0)),
        _ => FieldType::Text,
    }
}

fn load_event_type_for_apply(
    conn: &Connection,
    event_type_id: &str,
) -> Result<Option<EventType>, EventApplyRepoError> {
    let version: Option<i64> = conn
        .query_row(
            "SELECT current_version FROM event_types WHERE event_type_id = ?1",
            [event_type_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(version) = version else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "
        SELECT
            m.operation,
            td.tag_key,
            td.value_type,
            td.external_entity_type_id,
            m.input_key
        FROM event_type_mutations m
        JOIN tag_definitions td ON td.id = m.tag_definition_id
        WHERE m.event_type_id = ?1 AND m.event_type_version = ?2
        ORDER BY m.mutation_index
        ",
    )?;

    let mut rows = stmt.query(params![event_type_id, version])?;
    let mut mutations = Vec::new();
    while let Some(row) = rows.next()? {
        let operation: String = row.get(0)?;
        let field_id: String = row.get(1)?;
        let value_type: String = row.get(2)?;
        let ext_type: Option<i64> = row.get(3)?;
        let input_key: Option<String> = row.get(4)?;
        let field_type = parse_field_type(&value_type, ext_type);

        let mutation = match operation.as_str() {
            "set" => MutationSpec::Set {
                field_id,
                field_type,
                input_key: input_key.unwrap_or_else(|| "value".to_string()),
            },
            "clear" => MutationSpec::Clear { field_id },
            "increment" => MutationSpec::Increment {
                field_id,
                field_type,
                input_key: input_key.unwrap_or_else(|| "delta".to_string()),
            },
            _ => return Err(EventApplyRepoError::EventTypeMutationInvalid(operation)),
        };
        mutations.push(mutation);
    }

    Ok(Some(EventType {
        event_type_id: event_type_id.to_string(),
        event_type_version: u32::try_from(version).unwrap_or(1),
        mutations,
    }))
}

fn load_asset_state(conn: &Connection, asset_id: i64) -> Result<DomainState, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "
        SELECT td.tag_key, acv.value_json
        FROM asset_current_tag_values acv
        JOIN tag_definitions td ON td.id = acv.tag_definition_id
        WHERE acv.asset_id = ?1
        ",
    )?;
    let mut rows = stmt.query([asset_id])?;
    let mut out = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let value_json: String = row.get(1)?;
        if let Ok(value) = serde_json::from_str::<Value>(&value_json) {
            out.insert(key, value);
        }
    }
    Ok(out)
}

fn persist_asset_state(
    conn: &Connection,
    asset_id: i64,
    state: &DomainState,
) -> Result<(), EventApplyRepoError> {
    let mut map: HashMap<String, PersistTagMeta> = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT id, tag_key, value_type, external_entity_type_id FROM tag_definitions")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let key: String = row.get(1)?;
        map.insert(
            key,
            PersistTagMeta {
                tag_definition_id: row.get(0)?,
                value_type: row.get(2)?,
                external_entity_type_id: row.get(3)?,
            },
        );
    }

    conn.execute(
        "DELETE FROM asset_current_tag_values WHERE asset_id = ?1",
        [asset_id],
    )?;
    for (field_id, value) in state {
        if let Some(tag_meta) = map.get(field_id) {
            let (enum_option_id, external_entity_id) =
                resolve_value_references(conn, field_id, value, tag_meta)?;
            conn.execute(
                "
                INSERT INTO asset_current_tag_values (
                    asset_id,
                    tag_definition_id,
                    value_json,
                    enum_option_id,
                    external_entity_id
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    asset_id,
                    tag_meta.tag_definition_id,
                    serde_json::to_string(value).unwrap_or("null".to_string()),
                    enum_option_id,
                    external_entity_id
                ],
            )?;
        }
    }

    Ok(())
}

fn json_object(value: Value) -> Result<Map<String, Value>, EventApplyRepoError> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(EventApplyRepoError::InvalidPayload(
            "payload must be an object".to_string(),
        )),
    }
}

fn event_apply_message(err: &EventApplyError) -> String {
    err.to_string()
}

fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut sorted: Vec<_> = obj.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = Map::new();
            for (k, v) in sorted {
                out.insert(k.clone(), canonicalize_json_value(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json_value).collect()),
        _ => value.clone(),
    }
}

fn idempotency_scope_hash(
    method: &str,
    path: &str,
    asset_id: i64,
    canonical_payload: &str,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    method.hash(&mut hasher);
    path.hash(&mut hasher);
    asset_id.hash(&mut hasher);
    canonical_payload.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn encode_cursor(timestamp: &str, event_id: i64) -> String {
    let _ = timestamp;
    event_id.to_string()
}

fn decode_cursor(cursor: &str) -> Option<i64> {
    cursor.parse::<i64>().ok()
}

fn is_supported_operation(operation: &str) -> bool {
    matches!(operation, "set" | "clear" | "increment")
}

#[derive(Debug, Clone)]
struct PersistTagMeta {
    tag_definition_id: i64,
    value_type: String,
    external_entity_type_id: Option<i64>,
}

fn resolve_value_references(
    conn: &Connection,
    field_id: &str,
    value: &Value,
    tag_meta: &PersistTagMeta,
) -> Result<(Option<i64>, Option<i64>), EventApplyRepoError> {
    match tag_meta.value_type.as_str() {
        "enum" => {
            let option_key = value.as_str().ok_or_else(|| {
                EventApplyRepoError::InvalidPayload(format!(
                    "enum field `{field_id}` requires string option key"
                ))
            })?;

            let enum_option_id = conn
                .query_row(
                    "SELECT id FROM tag_enum_options WHERE tag_definition_id = ?1 AND option_key = ?2",
                    params![tag_meta.tag_definition_id, option_key],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .ok_or_else(|| EventApplyRepoError::EnumOptionNotFound {
                    field_id: field_id.to_string(),
                    option_key: option_key.to_string(),
                })?;

            Ok((Some(enum_option_id), None))
        }
        "external_entity" => {
            let entity_id = value.as_i64().ok_or_else(|| {
                EventApplyRepoError::InvalidPayload(format!(
                    "external_entity field `{field_id}` requires integer id"
                ))
            })?;

            let found_type_id = conn
                .query_row(
                    "SELECT external_entity_type_id FROM external_entities WHERE id = ?1",
                    [entity_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .ok_or_else(|| EventApplyRepoError::ExternalEntityNotFound {
                    field_id: field_id.to_string(),
                    entity_id,
                })?;

            let expected_type_id = tag_meta.external_entity_type_id.ok_or_else(|| {
                EventApplyRepoError::ExternalEntityTypeMissing(field_id.to_string())
            })?;

            if found_type_id != expected_type_id {
                return Err(EventApplyRepoError::ExternalEntityTypeMismatch {
                    field_id: field_id.to_string(),
                    entity_id,
                    expected_type_id,
                    found_type_id,
                });
            }

            Ok((None, Some(entity_id)))
        }
        _ => Ok((None, None)),
    }
}
