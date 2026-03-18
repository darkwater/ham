use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::tempdir;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[test]
fn db_schema_applies() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();

    assert_table_exists(&conn, "categories");
    assert_table_exists(&conn, "assets");
    assert_table_exists(&conn, "tag_definitions");
    assert_table_exists(&conn, "tag_enum_options");
    assert_table_exists(&conn, "category_tag_hints");
    assert_table_exists(&conn, "external_entity_types");
    assert_table_exists(&conn, "external_entities");
    assert_table_exists(&conn, "event_types");
    assert_table_exists(&conn, "event_type_mutations");
    assert_table_exists(&conn, "asset_current_tag_values");
    assert_table_exists(&conn, "asset_events");
    assert_table_exists(&conn, "tag_generator_settings");
    assert_table_exists(&conn, "tag_generator_counters");

    assert_column_exists(&conn, "event_types", "current_version");
    assert_column_exists(&conn, "event_type_mutations", "event_type_version");
    assert_column_exists(&conn, "asset_events", "event_type_version");

    assert_unique_index_has_columns(&conn, "assets", &["asset_tag"]);
    assert_unique_index_has_columns(&conn, "asset_events", &["asset_id", "idempotency_key"]);
}

#[test]
fn duplicate_manual_tag_returns_deterministic_error() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");
    let mut conn = server::db::open_and_prepare(&db_path).unwrap();

    insert_category(&conn, 1, "Network");
    set_tag_generator_settings(&conn, "AST", 4, "-");

    let first = server::db::repo_assets::create_asset(&mut conn, 1, Some("MANUAL-001")).unwrap();
    assert_eq!(first.asset_tag, "MANUAL-001");

    let err = server::db::repo_assets::create_asset(&mut conn, 1, Some("MANUAL-001")).unwrap_err();

    assert!(matches!(
        err,
        server::db::repo_assets::AssetCreateError::DuplicateAssetTag(tag)
        if tag == "MANUAL-001"
    ));
}

#[test]
fn startup_fails_on_incompatible_schema_version() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");

    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch("PRAGMA user_version = 999;").unwrap();

    let err = server::db::open_and_prepare(&db_path).unwrap_err();
    let message = err.to_string();

    assert!(message.contains("incompatible schema version"));
    assert!(message.contains("999"));
}

#[test]
fn event_type_mutation_uniqueness_is_scoped_by_version() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_for_mutation_and_event_fk(&conn);

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
        ("asset.update", 1_i64, 0_i64, "set", 1_i64, "label"),
    )
    .unwrap();

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
        ("asset.update", 2_i64, 0_i64, "set", 1_i64, "label"),
    )
    .unwrap();

    let err = conn
        .execute(
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
            ("asset.update", 2_i64, 0_i64, "set", 1_i64, "label"),
        )
        .unwrap_err();

    assert!(err
        .to_string()
        .contains("UNIQUE constraint failed: event_type_mutations.event_type_id, event_type_mutations.event_type_version, event_type_mutations.mutation_index"));
}

#[test]
fn migration_two_backfills_existing_rows_to_version_one() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");

    let conn = Connection::open(&db_path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(include_str!("../migrations/0001_initial.sql"))
        .unwrap();
    conn.pragma_update(None, "user_version", 1_i64).unwrap();
    seed_for_mutation_and_event_fk(&conn);

    conn.execute(
        "
        INSERT INTO event_type_mutations (
            event_type_id,
            mutation_index,
            operation,
            tag_definition_id,
            input_key
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        ",
        ("asset.update", 0_i64, "set", 1_i64, "label"),
    )
    .unwrap();

    conn.execute(
        "
        INSERT INTO asset_events (
            asset_id,
            idempotency_key,
            event_type_id,
            payload_json
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        (1_i64, "idem-1", "asset.update", "{}"),
    )
    .unwrap();
    drop(conn);

    let migrated = server::db::open_and_prepare(&db_path).unwrap();

    let current_version: i64 = migrated
        .query_row(
            "SELECT current_version FROM event_types WHERE event_type_id = ?1",
            ["asset.update"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current_version, 1);

    let mutation_version: i64 = migrated
        .query_row(
            "
            SELECT event_type_version
            FROM event_type_mutations
            WHERE event_type_id = ?1 AND mutation_index = ?2
            ",
            ("asset.update", 0_i64),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(mutation_version, 1);

    let event_version: i64 = migrated
        .query_row(
            "
            SELECT event_type_version
            FROM asset_events
            WHERE asset_id = ?1 AND idempotency_key = ?2
            ",
            (1_i64, "idem-1"),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_version, 1);
}

#[test]
fn auto_generates_asset_tag() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");
    let mut conn = server::db::open_and_prepare(&db_path).unwrap();

    insert_category(&conn, 1, "Network");
    set_tag_generator_settings(&conn, "AST", 4, "-");

    let created = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();

    assert_eq!(created.asset_tag, "AST-0001");
}

#[test]
fn manual_override_does_not_advance_counter() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");
    let mut conn = server::db::open_and_prepare(&db_path).unwrap();

    insert_category(&conn, 1, "Network");
    set_tag_generator_settings(&conn, "AST", 4, "-");

    let first_auto = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();
    assert_eq!(first_auto.asset_tag, "AST-0001");

    let manual = server::db::repo_assets::create_asset(&mut conn, 1, Some("MANUAL-001")).unwrap();
    assert_eq!(manual.asset_tag, "MANUAL-001");

    let second_auto = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();
    assert_eq!(second_auto.asset_tag, "AST-0002");
}

#[test]
fn generator_retries_on_collision() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");
    let mut conn = server::db::open_and_prepare(&db_path).unwrap();

    insert_category(&conn, 1, "Network");
    set_tag_generator_settings(&conn, "AST", 4, "-");
    set_global_counter(&conn, 1);

    conn.execute(
        "INSERT INTO assets (category_id, asset_tag) VALUES (?1, ?2)",
        (1_i64, "AST-0001"),
    )
    .unwrap();

    let first_auto = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();
    assert_eq!(first_auto.asset_tag, "AST-0002");

    let second_auto = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();
    assert_eq!(second_auto.asset_tag, "AST-0003");
}

#[test]
fn auto_generation_counter_is_global_across_categories() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets.db");
    let mut conn = server::db::open_and_prepare(&db_path).unwrap();

    insert_category(&conn, 1, "Network");
    insert_category(&conn, 2, "Compute");
    set_tag_generator_settings(&conn, "AST", 4, "-");

    let first = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();
    let second = server::db::repo_assets::create_asset(&mut conn, 2, None).unwrap();
    let third = server::db::repo_assets::create_asset(&mut conn, 1, None).unwrap();

    assert_eq!(first.asset_tag, "AST-0001");
    assert_eq!(second.asset_tag, "AST-0002");
    assert_eq!(third.asset_tag, "AST-0003");
}

fn assert_table_exists(conn: &Connection, table: &str) {
    let mut stmt = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .unwrap();
    let exists = stmt.exists([table]).unwrap();
    assert!(exists, "expected table `{table}` to exist");
}

fn assert_column_exists(conn: &Connection, table: &str, column: &str) {
    let pragma = format!("PRAGMA table_info('{table}')");
    let mut stmt = conn.prepare(&pragma).unwrap();
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        names.iter().any(|name| name == column),
        "expected column `{column}` to exist on table `{table}`"
    );
}

fn assert_unique_index_has_columns(conn: &Connection, table: &str, columns: &[&str]) {
    let sql = "
        SELECT il.name
        FROM pragma_index_list(?1) il
        WHERE il.[unique] = 1
    ";

    let mut stmt = conn.prepare(sql).unwrap();
    let mut rows = stmt.query([table]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        let index_name: String = row.get(0).unwrap();
        let pragma = format!("PRAGMA index_info('{index_name}')");
        let mut idx_stmt = conn.prepare(&pragma).unwrap();
        let idx_columns = idx_stmt
            .query_map([], |r| r.get::<_, String>(2))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        if idx_columns == columns {
            return;
        }
    }

    panic!(
        "no unique index on table `{table}` with columns {:?}",
        columns
    );
}

fn seed_for_mutation_and_event_fk(conn: &Connection) {
    conn.execute(
        "INSERT OR IGNORE INTO categories (id, name) VALUES (?1, ?2)",
        (1_i64, "Network"),
    )
    .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO assets (id, category_id, asset_tag) VALUES (?1, ?2, ?3)",
        (1_i64, 1_i64, "AST-0001"),
    )
    .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO event_types (event_type_id, display_name) VALUES (?1, ?2)",
        ("asset.update", "Asset Update"),
    )
    .unwrap();
    conn.execute(
        "
        INSERT OR IGNORE INTO tag_definitions (id, tag_key, display_name, value_type)
        VALUES (?1, ?2, ?3, ?4)
        ",
        (1_i64, "label", "Label", "text"),
    )
    .unwrap();
}

fn insert_category(conn: &Connection, id: i64, name: &str) {
    conn.execute(
        "INSERT INTO categories (id, name) VALUES (?1, ?2)",
        (id, name),
    )
    .unwrap();
}

fn set_tag_generator_settings(conn: &Connection, prefix: &str, width: i64, separator: &str) {
    conn.execute(
        "
        UPDATE tag_generator_settings
        SET prefix = ?1,
            number_width = ?2,
            separator = ?3,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = 1
        ",
        (prefix, width, separator),
    )
    .unwrap();
}

fn set_global_counter(conn: &Connection, next_value: i64) {
    conn.execute(
        "
        UPDATE tag_generator_counters
        SET next_value = ?1,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = 1
        ",
        [next_value],
    )
    .unwrap();
}

#[tokio::test]
async fn asset_update_rejects_tag_value_changes() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets-http.db");

    let mut conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "Network");
    let created =
        server::db::repo_assets::create_asset(&mut conn, 1, Some("AST-HTTP-001")).unwrap();

    let app = server::app::build_app(db_path.clone()).unwrap();
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/assets/{}", created.id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "tag_values": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);

    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "ASSET_TAG_VALUES_UPDATE_FORBIDDEN");
}

#[tokio::test]
async fn asset_create_and_read_succeed() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "Network");

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/assets")
        .header("content-type", "application/json")
        .header("authorization", "Bearer ignored")
        .body(Body::from(
            json!({
                "category_id": 1,
                "asset_tag": "AST-HTTP-004"
            })
            .to_string(),
        ))
        .unwrap();
    let create_res = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let created = read_json(create_res.into_body()).await;
    assert_eq!(created["asset_tag"], "AST-HTTP-004");
    let id = created["id"].as_i64().unwrap();

    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/assets/{id}"))
        .body(Body::empty())
        .unwrap();
    let get_res = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
    let body = read_json(get_res.into_body()).await;
    assert_eq!(body["id"], id);
    assert_eq!(body["asset_tag"], "AST-HTTP-004");
}

#[tokio::test]
async fn metadata_update_on_active_asset_succeeds() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets-http.db");

    let mut conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "Network");
    let created =
        server::db::repo_assets::create_asset(&mut conn, 1, Some("AST-HTTP-005")).unwrap();

    let app = server::app::build_app(db_path).unwrap();

    let patch_req = Request::builder()
        .method("PATCH")
        .uri(format!("/assets/{}", created.id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "display_name": "Edge Router"
            })
            .to_string(),
        ))
        .unwrap();
    let patch_res = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_res.status(), StatusCode::NO_CONTENT);

    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/assets/{}", created.id))
        .body(Body::empty())
        .unwrap();
    let get_res = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
    let body = read_json(get_res.into_body()).await;
    assert_eq!(body["display_name"], "Edge Router");
}

#[tokio::test]
async fn deleted_assets_are_excluded_unless_include_deleted_true() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets-http.db");

    let mut conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "Network");
    let created =
        server::db::repo_assets::create_asset(&mut conn, 1, Some("AST-HTTP-002")).unwrap();

    let app = server::app::build_app(db_path.clone()).unwrap();

    let delete_req = Request::builder()
        .method("DELETE")
        .uri(format!("/assets/{}", created.id))
        .body(Body::empty())
        .unwrap();
    let delete_res = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    let list_default_req = Request::builder()
        .method("GET")
        .uri("/assets")
        .header("authorization", "Bearer ignored")
        .body(Body::empty())
        .unwrap();
    let list_default_res = app.clone().oneshot(list_default_req).await.unwrap();
    assert_eq!(list_default_res.status(), StatusCode::OK);
    let default_body = read_json(list_default_res.into_body()).await;
    assert_eq!(default_body["items"], json!([]));

    let list_including_deleted_req = Request::builder()
        .method("GET")
        .uri("/assets?include_deleted=true")
        .body(Body::empty())
        .unwrap();
    let list_including_deleted_res = app.oneshot(list_including_deleted_req).await.unwrap();
    assert_eq!(list_including_deleted_res.status(), StatusCode::OK);
    let include_deleted_body = read_json(list_including_deleted_res.into_body()).await;
    assert_eq!(include_deleted_body["items"].as_array().unwrap().len(), 1);
    assert_eq!(include_deleted_body["items"][0]["id"], created.id);
}

#[tokio::test]
async fn metadata_update_on_deleted_asset_returns_gone() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("assets-http.db");

    let mut conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "Network");
    let created =
        server::db::repo_assets::create_asset(&mut conn, 1, Some("AST-HTTP-003")).unwrap();

    let app = server::app::build_app(db_path.clone()).unwrap();

    let delete_req = Request::builder()
        .method("DELETE")
        .uri(format!("/assets/{}", created.id))
        .body(Body::empty())
        .unwrap();
    let delete_res = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    let patch_req = Request::builder()
        .method("PATCH")
        .uri(format!("/assets/{}", created.id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "display_name": "new-name"
            })
            .to_string(),
        ))
        .unwrap();
    let patch_res = app.oneshot(patch_req).await.unwrap();
    assert_eq!(patch_res.status(), StatusCode::GONE);

    let body = read_json(patch_res.into_body()).await;
    assert_eq!(body["reason_code"], "ASSET_SOFT_DELETED");
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
