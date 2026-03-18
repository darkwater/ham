use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn event_type_version_lifecycle_endpoints_round_trip() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "description": "Assign owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let create_res = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let created = read_json(create_res.into_body()).await;
    assert_eq!(created["event_type_id"], "asset.set-owner");
    assert_eq!(created["version"], 1);

    let create_v2_req = Request::builder()
        .method("POST")
        .uri("/event-types/asset.set-owner/versions")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner_name"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let create_v2_res = app.clone().oneshot(create_v2_req).await.unwrap();
    assert_eq!(create_v2_res.status(), StatusCode::CREATED);
    let created_v2 = read_json(create_v2_res.into_body()).await;
    assert_eq!(created_v2["version"], 2);
    assert_eq!(created_v2["mutations"][0]["input_key"], "owner_name");

    let get_current_req = Request::builder()
        .method("GET")
        .uri("/event-types/asset.set-owner")
        .body(Body::empty())
        .unwrap();
    let get_current_res = app.clone().oneshot(get_current_req).await.unwrap();
    assert_eq!(get_current_res.status(), StatusCode::OK);
    let current = read_json(get_current_res.into_body()).await;
    assert_eq!(current["version"], 2);
    assert_eq!(current["mutations"][0]["input_key"], "owner_name");

    let get_v1_req = Request::builder()
        .method("GET")
        .uri("/event-types/asset.set-owner/versions/1")
        .body(Body::empty())
        .unwrap();
    let get_v1_res = app.oneshot(get_v1_req).await.unwrap();
    assert_eq!(get_v1_res.status(), StatusCode::OK);
    let v1 = read_json(get_v1_res.into_body()).await;
    assert_eq!(v1["version"], 1);
    assert_eq!(v1["mutations"][0]["input_key"], "owner");
}

#[tokio::test]
async fn referenced_event_type_version_cannot_be_deleted() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-001");

    let app = server::app::build_app(db_path.clone()).unwrap();

    let create_event_type_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let create_event_type_res = app.clone().oneshot(create_event_type_req).await.unwrap();
    assert_eq!(create_event_type_res.status(), StatusCode::CREATED);

    let create_event_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-001/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-a")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let create_event_res = app.clone().oneshot(create_event_req).await.unwrap();
    assert_eq!(create_event_res.status(), StatusCode::CREATED);

    let delete_version_req = Request::builder()
        .method("DELETE")
        .uri("/event-types/asset.set-owner/versions/1")
        .body(Body::empty())
        .unwrap();
    let delete_version_res = app.oneshot(delete_version_req).await.unwrap();
    assert_eq!(delete_version_res.status(), StatusCode::CONFLICT);
    let body = read_json(delete_version_res.into_body()).await;
    assert_eq!(body["reason_code"], "EVENT_TYPE_VERSION_IN_USE");

    let version_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM event_type_mutations WHERE event_type_id = ?1 AND event_type_version = ?2",
            ("asset.set-owner", 1_i64),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version_count, 1);
}

#[tokio::test]
async fn deleted_event_type_version_returns_not_found_on_fetch() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(create_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let create_v2_req = Request::builder()
        .method("POST")
        .uri("/event-types/asset.set-owner/versions")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner_name"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(create_v2_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let delete_v1_req = Request::builder()
        .method("DELETE")
        .uri("/event-types/asset.set-owner/versions/1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(delete_v1_req).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );

    let get_v1_req = Request::builder()
        .method("GET")
        .uri("/event-types/asset.set-owner/versions/1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(get_v1_req).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn create_asset_event_enforces_idempotency_key_and_timestamp_rules() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-002");

    let app = server::app::build_app(db_path).unwrap();

    let create_event_type_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let create_event_type_res = app.clone().oneshot(create_event_type_req).await.unwrap();
    assert_eq!(create_event_type_res.status(), StatusCode::CREATED);

    let missing_key_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-002/events")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let missing_key_res = app.clone().oneshot(missing_key_req).await.unwrap();
    assert_eq!(missing_key_res.status(), StatusCode::BAD_REQUEST);
    let missing_key_body = read_json(missing_key_res.into_body()).await;
    assert_eq!(missing_key_body["reason_code"], "IDEMPOTENCY_KEY_REQUIRED");

    let timestamp_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-002/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-ts")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "timestamp": "2024-01-01T00:00:00Z",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let timestamp_res = app.oneshot(timestamp_req).await.unwrap();
    assert_eq!(timestamp_res.status(), StatusCode::BAD_REQUEST);
    let timestamp_body = read_json(timestamp_res.into_body()).await;
    assert_eq!(timestamp_body["reason_code"], "EVENT_TIMESTAMP_FORBIDDEN");
}

#[tokio::test]
async fn create_asset_event_supports_idempotent_replay_and_payload_mismatch_conflict() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-003");

    let app = server::app::build_app(db_path).unwrap();

    let create_event_type_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let create_event_type_res = app.clone().oneshot(create_event_type_req).await.unwrap();
    assert_eq!(create_event_type_res.status(), StatusCode::CREATED);

    let first_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-003/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-1")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let first_res = app.clone().oneshot(first_req).await.unwrap();
    assert_eq!(first_res.status(), StatusCode::CREATED);
    let first_body = read_json(first_res.into_body()).await;
    let first_event_id = first_body["event_id"].as_i64().unwrap();

    let replay_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-003/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-1")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let replay_res = app.clone().oneshot(replay_req).await.unwrap();
    assert_eq!(replay_res.status(), StatusCode::OK);
    let replay_body = read_json(replay_res.into_body()).await;
    assert_eq!(replay_body["event_id"], first_event_id);
    assert_eq!(replay_body["replayed"], true);

    let mismatch_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-003/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-1")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {
                    "owner": "team-b"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let mismatch_res = app.oneshot(mismatch_req).await.unwrap();
    assert_eq!(mismatch_res.status(), StatusCode::CONFLICT);
    let mismatch_body = read_json(mismatch_res.into_body()).await;
    assert_eq!(
        mismatch_body["reason_code"],
        "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH"
    );
}

#[tokio::test]
async fn event_type_creation_rejects_empty_mutations() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");
    let _conn = server::db::open_and_prepare(&db_path).unwrap();

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.empty",
                "display_name": "Empty",
                "mutations": []
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(create_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "EVENT_TYPE_MUTATIONS_REQUIRED");
}

#[tokio::test]
async fn event_type_new_version_rejects_empty_mutations() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(create_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let create_v2_req = Request::builder()
        .method("POST")
        .uri("/event-types/asset.set-owner/versions")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "mutations": [] }).to_string()))
        .unwrap();
    let res = app.oneshot(create_v2_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "EVENT_TYPE_MUTATIONS_REQUIRED");
}

#[tokio::test]
async fn event_type_creation_rejects_unsupported_mutation_operation() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.bad-op",
                "display_name": "Bad Op",
                "mutations": [
                    {
                        "operation": "multiply",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(create_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "UNSUPPORTED_MUTATION_OPERATION");
}

#[tokio::test]
async fn event_type_new_version_rejects_unsupported_mutation_operation() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");

    let app = server::app::build_app(db_path).unwrap();

    let create_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(create_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let create_v2_req = Request::builder()
        .method("POST")
        .uri("/event-types/asset.set-owner/versions")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "mutations": [
                    {
                        "operation": "multiply",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(create_v2_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "UNSUPPORTED_MUTATION_OPERATION");
}

#[tokio::test]
async fn apply_event_fails_when_event_type_contains_unknown_operation() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-UNKNOWN-OP");
    conn.execute(
        "INSERT INTO event_types (event_type_id, display_name, current_version) VALUES (?1, ?2, ?3)",
        ("asset.bad-op", "Bad Op", 1_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO event_type_mutations (event_type_id, event_type_version, mutation_index, operation, tag_definition_id, input_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        ("asset.bad-op", 1_i64, 0_i64, "multiply", 1_i64, "owner"),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-UNKNOWN-OP/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-unknown-op")
        .body(Body::from(
            json!({
                "event_type_id": "asset.bad-op",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "EVENT_TYPE_MUTATION_INVALID");
}

#[tokio::test]
async fn create_asset_event_persists_enum_and_external_entity_foreign_keys() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "status", "Status", "enum");
    seed_tag_definition(&conn, 2, "vendor_id", "Vendor", "external_entity");
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        (10_i64, "vendor", "Vendor"),
    )
    .unwrap();
    conn.execute(
        "UPDATE tag_definitions SET external_entity_type_id = ?1 WHERE id = ?2",
        (10_i64, 2_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_enum_options (id, tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        (21_i64, 1_i64, "active", "Active", 0_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entities (id, external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3, ?4)",
        (31_i64, 10_i64, "v-31", "Vendor 31"),
    )
    .unwrap();
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-006");

    let app = server::app::build_app(db_path.clone()).unwrap();

    let create_event_type_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "display_name": "Classify",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "status"
                    },
                    {
                        "operation": "set",
                        "tag_definition_id": 2,
                        "input_key": "vendor_id"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone()
            .oneshot(create_event_type_req)
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    let apply_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-006/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-fk")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "payload": {
                    "status": "active",
                    "vendor_id": 31
                }
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(apply_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let (enum_option_id, external_entity_id): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT enum_option_id, external_entity_id FROM asset_current_tag_values WHERE asset_id = ?1 AND tag_definition_id = ?2",
            (1_i64, 1_i64),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(enum_option_id, Some(21));
    assert_eq!(external_entity_id, None);

    let (enum_option_id, external_entity_id): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT enum_option_id, external_entity_id FROM asset_current_tag_values WHERE asset_id = ?1 AND tag_definition_id = ?2",
            (1_i64, 2_i64),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(enum_option_id, None);
    assert_eq!(external_entity_id, Some(31));
}

#[tokio::test]
async fn create_asset_event_rejects_invalid_enum_or_external_entity_reference() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "status", "Status", "enum");
    seed_tag_definition(&conn, 2, "vendor_id", "Vendor", "external_entity");
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        (10_i64, "vendor", "Vendor"),
    )
    .unwrap();
    conn.execute(
        "UPDATE tag_definitions SET external_entity_type_id = ?1 WHERE id = ?2",
        (10_i64, 2_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_enum_options (id, tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        (21_i64, 1_i64, "active", "Active", 0_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        (11_i64, "site", "Site"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entities (id, external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3, ?4)",
        (31_i64, 11_i64, "s-31", "Site 31"),
    )
    .unwrap();
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-007");

    let app = server::app::build_app(db_path.clone()).unwrap();

    let create_event_type_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "display_name": "Classify",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "status"
                    },
                    {
                        "operation": "set",
                        "tag_definition_id": 2,
                        "input_key": "vendor_id"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone()
            .oneshot(create_event_type_req)
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    let bad_enum_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-007/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-invalid-enum")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "payload": {
                    "status": "missing",
                    "vendor_id": 31
                }
            })
            .to_string(),
        ))
        .unwrap();
    let bad_enum_res = app.clone().oneshot(bad_enum_req).await.unwrap();
    assert_eq!(bad_enum_res.status(), StatusCode::BAD_REQUEST);
    let bad_enum_body = read_json(bad_enum_res.into_body()).await;
    assert_eq!(bad_enum_body["reason_code"], "INVALID_EVENT_PAYLOAD");

    let bad_external_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-007/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-invalid-external")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "payload": {
                    "status": "active",
                    "vendor_id": 31
                }
            })
            .to_string(),
        ))
        .unwrap();
    let bad_external_res = app.clone().oneshot(bad_external_req).await.unwrap();
    assert_eq!(bad_external_res.status(), StatusCode::CONFLICT);
    let bad_external_body = read_json(bad_external_res.into_body()).await;
    assert_eq!(
        bad_external_body["reason_code"],
        "EVENT_VALUE_REFERENCE_CONFLICT"
    );

    let event_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM asset_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(event_count, 0);
}

#[tokio::test]
async fn create_asset_event_applies_projection_atomically() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition(&conn, 1, "owner", "Owner", "text");
    seed_tag_definition(&conn, 2, "count", "Count", "integer");
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-004");

    let app = server::app::build_app(db_path.clone()).unwrap();

    let create_event_type_req = Request::builder()
        .method("POST")
        .uri("/event-types")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "display_name": "Set Owner",
                "mutations": [
                    {
                        "operation": "set",
                        "tag_definition_id": 1,
                        "input_key": "owner"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone()
            .oneshot(create_event_type_req)
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    let ok_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-004/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-ok")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {
                    "owner": "team-a"
                }
            })
            .to_string(),
        ))
        .unwrap();
    let ok_res = app.clone().oneshot(ok_req).await.unwrap();
    assert_eq!(ok_res.status(), StatusCode::CREATED);

    let event_count_after_ok: i64 = conn
        .query_row("SELECT COUNT(*) FROM asset_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(event_count_after_ok, 1);
    let projection_after_ok: String = conn
        .query_row(
            "SELECT value_json FROM asset_current_tag_values WHERE asset_id = ?1 AND tag_definition_id = ?2",
            (1_i64, 1_i64),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(projection_after_ok, "\"team-a\"");

    let bad_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-EVENT-004/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-bad")
        .body(Body::from(
            json!({
                "event_type_id": "asset.set-owner",
                "payload": {}
            })
            .to_string(),
        ))
        .unwrap();
    let bad_res = app.oneshot(bad_req).await.unwrap();
    assert_eq!(bad_res.status(), StatusCode::BAD_REQUEST);

    let event_count_after_bad: i64 = conn
        .query_row("SELECT COUNT(*) FROM asset_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(event_count_after_bad, 1);
    let projection_after_bad: String = conn
        .query_row(
            "SELECT value_json FROM asset_current_tag_values WHERE asset_id = ?1 AND tag_definition_id = ?2",
            (1_i64, 1_i64),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(projection_after_bad, "\"team-a\"");
}

#[tokio::test]
async fn list_asset_events_uses_desc_order_and_stable_cursor() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_category(&conn, 1, "Network");
    seed_asset(&conn, 1, 1, "AST-EVENT-005");
    conn.execute(
        "INSERT INTO event_types (event_type_id, display_name, current_version) VALUES (?1, ?2, ?3)",
        ("asset.manual", "Manual", 1_i64),
    )
    .unwrap();

    conn.execute(
        "
        INSERT INTO asset_events (id, asset_id, idempotency_key, event_type_id, event_type_version, payload_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        (
            100_i64,
            1_i64,
            "k-100",
            "asset.manual",
            1_i64,
            "{}",
            "2024-01-01 00:00:00",
        ),
    )
    .unwrap();
    conn.execute(
        "
        INSERT INTO asset_events (id, asset_id, idempotency_key, event_type_id, event_type_version, payload_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        (
            101_i64,
            1_i64,
            "k-101",
            "asset.manual",
            1_i64,
            "{}",
            "2024-01-01 00:00:00",
        ),
    )
    .unwrap();
    conn.execute(
        "
        INSERT INTO asset_events (id, asset_id, idempotency_key, event_type_id, event_type_version, payload_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        (
            102_i64,
            1_i64,
            "k-102",
            "asset.manual",
            1_i64,
            "{}",
            "2024-01-01 00:00:00",
        ),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();

    let first_page_req = Request::builder()
        .method("GET")
        .uri("/assets/AST-EVENT-005/events?limit=2")
        .body(Body::empty())
        .unwrap();
    let first_page_res = app.clone().oneshot(first_page_req).await.unwrap();
    assert_eq!(first_page_res.status(), StatusCode::OK);
    let first_page = read_json(first_page_res.into_body()).await;
    assert_eq!(first_page["items"].as_array().unwrap().len(), 2);
    assert_eq!(first_page["items"][0]["event_id"], 102);
    assert_eq!(first_page["items"][1]["event_id"], 101);
    let cursor = first_page["next_cursor"].as_str().unwrap().to_string();

    let second_page_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/assets/AST-EVENT-005/events?limit=2&cursor={cursor}"
        ))
        .body(Body::empty())
        .unwrap();
    let second_page_res = app.oneshot(second_page_req).await.unwrap();
    assert_eq!(second_page_res.status(), StatusCode::OK);
    let second_page = read_json(second_page_res.into_body()).await;
    assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
    assert_eq!(second_page["items"][0]["event_id"], 100);
}

#[tokio::test]
async fn deleted_asset_timeline_returns_not_found() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("events-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_category(&conn, 1, "Network");
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag, deleted_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
        (1_i64, 1_i64, "AST-EVENT-DELETED"),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();
    let req = Request::builder()
        .method("GET")
        .uri("/assets/AST-EVENT-DELETED/events")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn seed_category(conn: &Connection, id: i64, name: &str) {
    conn.execute(
        "INSERT INTO categories (id, name) VALUES (?1, ?2)",
        (id, name),
    )
    .unwrap();
}

fn seed_asset(conn: &Connection, id: i64, category_id: i64, asset_tag: &str) {
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag) VALUES (?1, ?2, ?3)",
        (id, category_id, asset_tag),
    )
    .unwrap();
}

fn seed_tag_definition(
    conn: &Connection,
    id: i64,
    tag_key: &str,
    display_name: &str,
    value_type: &str,
) {
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (id, tag_key, display_name, value_type),
    )
    .unwrap();
}
