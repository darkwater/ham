use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn category_delete_blocked_by_child_categories() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "root", "Root", None);
    insert_category(&conn, 2, "child", "Child", Some(1));

    let app = server::app::build_app(db_path).unwrap();
    let req = Request::builder()
        .method("DELETE")
        .uri("/categories/1")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "CATEGORY_HAS_CHILDREN");
}

#[tokio::test]
async fn categories_list_returns_deterministic_ordering_and_expected_fields() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "b-root", "Root", None);
    insert_category(&conn, 2, "a-child", "Child", Some(1));
    insert_category(&conn, 3, "z-leaf", "Leaf", Some(1));

    let app = server::app::build_app(db_path).unwrap();
    let req = Request::builder()
        .method("GET")
        .uri("/categories")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;

    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["id"], 2);
    assert_eq!(items[0]["slug"], "a-child");
    assert_eq!(items[0]["name"], "Child");
    assert_eq!(items[0]["parent_category_id"], 1);

    assert_eq!(items[1]["id"], 1);
    assert_eq!(items[1]["slug"], "b-root");
    assert_eq!(items[1]["name"], "Root");
    assert!(items[1]["parent_category_id"].is_null());

    assert_eq!(items[2]["id"], 3);
    assert_eq!(items[2]["slug"], "z-leaf");
    assert_eq!(items[2]["name"], "Leaf");
    assert_eq!(items[2]["parent_category_id"], 1);
}

#[tokio::test]
async fn category_delete_blocked_by_assigned_assets() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let mut conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "network", "Network", None);
    let _ = server::db::repo_assets::create_asset(&mut conn, 1, Some("AST-CAT-001")).unwrap();

    let app = server::app::build_app(db_path).unwrap();
    let req = Request::builder()
        .method("DELETE")
        .uri("/categories/1")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "CATEGORY_HAS_ASSETS");
}

#[tokio::test]
async fn inherited_category_tag_hints_are_returned() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "root", "Root", None);
    insert_category(&conn, 2, "leaf", "Leaf", Some(1));

    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (1_i64, "owner", "Owner", "text"),
    )
    .unwrap();

    let app = server::app::build_app(db_path.clone()).unwrap();
    let create_hint_req = Request::builder()
        .method("PUT")
        .uri("/categories/1/tag-hints/1")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "is_required": true,
                "sort_order": 10
            })
            .to_string(),
        ))
        .unwrap();
    let create_hint_res = app.clone().oneshot(create_hint_req).await.unwrap();
    assert_eq!(create_hint_res.status(), StatusCode::OK);

    let inherited_lookup_req = Request::builder()
        .method("GET")
        .uri("/categories/2/tag-hints?inherited=true")
        .body(Body::empty())
        .unwrap();
    let inherited_lookup_res = app.oneshot(inherited_lookup_req).await.unwrap();
    assert_eq!(inherited_lookup_res.status(), StatusCode::OK);
    let body = read_json(inherited_lookup_res.into_body()).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["category_id"], 1);
    assert_eq!(body["items"][0]["tag_definition_id"], 1);
}

#[tokio::test]
async fn category_tag_hint_create_list_and_delete_round_trip() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "root", "Root", None);
    insert_category(&conn, 2, "leaf", "Leaf", Some(1));
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (1_i64, "owner", "Owner", "text"),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();

    let create_hint_req = Request::builder()
        .method("PUT")
        .uri("/categories/1/tag-hints/1")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "is_required": false,
                "sort_order": 1
            })
            .to_string(),
        ))
        .unwrap();
    let create_hint_res = app.clone().oneshot(create_hint_req).await.unwrap();
    assert_eq!(create_hint_res.status(), StatusCode::OK);

    let inherited_before_delete_req = Request::builder()
        .method("GET")
        .uri("/categories/2/tag-hints?inherited=true")
        .body(Body::empty())
        .unwrap();
    let inherited_before_delete_res = app
        .clone()
        .oneshot(inherited_before_delete_req)
        .await
        .unwrap();
    assert_eq!(inherited_before_delete_res.status(), StatusCode::OK);
    let before_body = read_json(inherited_before_delete_res.into_body()).await;
    assert_eq!(before_body["items"].as_array().unwrap().len(), 1);

    let delete_hint_req = Request::builder()
        .method("DELETE")
        .uri("/categories/1/tag-hints/1")
        .body(Body::empty())
        .unwrap();
    let delete_hint_res = app.clone().oneshot(delete_hint_req).await.unwrap();
    assert_eq!(delete_hint_res.status(), StatusCode::NO_CONTENT);

    let inherited_after_delete_req = Request::builder()
        .method("GET")
        .uri("/categories/2/tag-hints?inherited=true")
        .body(Body::empty())
        .unwrap();
    let inherited_after_delete_res = app.oneshot(inherited_after_delete_req).await.unwrap();
    assert_eq!(inherited_after_delete_res.status(), StatusCode::OK);
    let after_body = read_json(inherited_after_delete_res.into_body()).await;
    assert_eq!(after_body["items"], json!([]));
}

#[tokio::test]
async fn tag_definition_lifecycle_blocks_when_referenced() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition_reference_graph(&conn);

    let app = server::app::build_app(db_path).unwrap();

    let patch_req = Request::builder()
        .method("PATCH")
        .uri("/tag-definitions/1")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "value_type": "integer" }).to_string()))
        .unwrap();
    let patch_res = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_res.status(), StatusCode::CONFLICT);
    let patch_body = read_json(patch_res.into_body()).await;
    assert_eq!(patch_body["reason_code"], "TAG_DEFINITION_TYPE_IN_USE");

    let delete_req = Request::builder()
        .method("DELETE")
        .uri("/tag-definitions/1")
        .body(Body::empty())
        .unwrap();
    let delete_res = app.oneshot(delete_req).await.unwrap();
    assert_eq!(delete_res.status(), StatusCode::CONFLICT);
    let delete_body = read_json(delete_res.into_body()).await;
    assert_eq!(delete_body["reason_code"], "TAG_DEFINITION_IN_USE");
}

#[tokio::test]
async fn external_entity_lifecycle_constraints_are_enforced() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_external_entity_reference_graph(&conn);

    let app = server::app::build_app(db_path).unwrap();

    let delete_type_req = Request::builder()
        .method("DELETE")
        .uri("/external-entity-types/1")
        .body(Body::empty())
        .unwrap();
    let delete_type_res = app.clone().oneshot(delete_type_req).await.unwrap();
    assert_eq!(delete_type_res.status(), StatusCode::CONFLICT);
    let delete_type_body = read_json(delete_type_res.into_body()).await;
    assert_eq!(
        delete_type_body["reason_code"],
        "EXTERNAL_ENTITY_TYPE_IN_USE"
    );

    let delete_entity_req = Request::builder()
        .method("DELETE")
        .uri("/external-entities/1")
        .body(Body::empty())
        .unwrap();
    let delete_entity_res = app.clone().oneshot(delete_entity_req).await.unwrap();
    assert_eq!(delete_entity_res.status(), StatusCode::CONFLICT);
    let delete_entity_body = read_json(delete_entity_res.into_body()).await;
    assert_eq!(delete_entity_body["reason_code"], "EXTERNAL_ENTITY_IN_USE");

    let delete_option_req = Request::builder()
        .method("DELETE")
        .uri("/tag-enum-options/1")
        .body(Body::empty())
        .unwrap();
    let delete_option_res = app.clone().oneshot(delete_option_req).await.unwrap();
    assert_eq!(delete_option_res.status(), StatusCode::CONFLICT);
    let delete_option_body = read_json(delete_option_res.into_body()).await;
    assert_eq!(delete_option_body["reason_code"], "TAG_ENUM_OPTION_IN_USE");

    let retire_option_req = Request::builder()
        .method("PATCH")
        .uri("/tag-enum-options/1/retire")
        .body(Body::empty())
        .unwrap();
    let retire_option_res = app.clone().oneshot(retire_option_req).await.unwrap();
    assert_eq!(retire_option_res.status(), StatusCode::NO_CONTENT);

    let is_active: i64 = conn
        .query_row(
            "SELECT is_active FROM tag_enum_options WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(is_active, 0);
}

#[tokio::test]
async fn event_applied_enum_and_external_entity_values_block_lifecycle_deletes() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    conn.execute(
        "INSERT INTO categories (id, slug, name) VALUES (?1, ?2, ?3)",
        (1_i64, "network", "Network"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag) VALUES (?1, ?2, ?3)",
        (1_i64, 1_i64, "AST-LIFE-001"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        (10_i64, "vendor", "Vendor"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entities (id, external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3, ?4)",
        (11_i64, 10_i64, "v-11", "Vendor 11"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (20_i64, "status", "Status", "enum"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type, external_entity_type_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        (21_i64, "vendor_id", "Vendor", "external_entity", 10_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_enum_options (id, tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        (30_i64, 20_i64, "active", "Active", 0_i64),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();

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
                        "tag_definition_id": 20,
                        "input_key": "status"
                    },
                    {
                        "operation": "set",
                        "tag_definition_id": 21,
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

    let apply_event_req = Request::builder()
        .method("POST")
        .uri("/assets/AST-LIFE-001/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-life")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "payload": {
                    "status": "active",
                    "vendor_id": 11
                }
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(apply_event_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let delete_entity_req = Request::builder()
        .method("DELETE")
        .uri("/external-entities/11")
        .body(Body::empty())
        .unwrap();
    let delete_entity_res = app.clone().oneshot(delete_entity_req).await.unwrap();
    assert_eq!(delete_entity_res.status(), StatusCode::CONFLICT);
    let delete_entity_body = read_json(delete_entity_res.into_body()).await;
    assert_eq!(delete_entity_body["reason_code"], "EXTERNAL_ENTITY_IN_USE");

    let delete_option_req = Request::builder()
        .method("DELETE")
        .uri("/tag-enum-options/30")
        .body(Body::empty())
        .unwrap();
    let delete_option_res = app.oneshot(delete_option_req).await.unwrap();
    assert_eq!(delete_option_res.status(), StatusCode::CONFLICT);
    let delete_option_body = read_json(delete_option_res.into_body()).await;
    assert_eq!(delete_option_body["reason_code"], "TAG_ENUM_OPTION_IN_USE");
}

#[tokio::test]
async fn category_delete_blocked_even_with_only_soft_deleted_assets() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    insert_category(&conn, 1, "network", "Network", None);
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag, deleted_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
        (1_i64, 1_i64, "AST-SOFT-001"),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();
    let req = Request::builder()
        .method("DELETE")
        .uri("/categories/1")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "CATEGORY_HAS_ASSETS");
}

#[tokio::test]
async fn tag_definition_delete_conflicts_for_other_reference_types() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    seed_tag_definition_non_event_references(&conn);

    let app = server::app::build_app(db_path).unwrap();

    for id in [10_i64, 11_i64, 12_i64] {
        let req = Request::builder()
            .method("DELETE")
            .uri(format!("/tag-definitions/{id}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        let body = read_json(res.into_body()).await;
        assert_eq!(body["reason_code"], "TAG_DEFINITION_IN_USE");
    }
}

#[tokio::test]
async fn external_entity_type_delete_conflicts_when_entities_exist_without_tag_defs() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let conn = server::db::open_and_prepare(&db_path).unwrap();
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        (7_i64, "site", "Site"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entities (id, external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3, ?4)",
        (7_i64, 7_i64, "site-7", "Site 7"),
    )
    .unwrap();

    let app = server::app::build_app(db_path).unwrap();
    let req = Request::builder()
        .method("DELETE")
        .uri("/external-entity-types/7")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["reason_code"], "EXTERNAL_ENTITY_TYPE_IN_USE");
}

#[tokio::test]
async fn missing_mutation_targets_return_not_found() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("categories-http.db");

    let _conn = server::db::open_and_prepare(&db_path).unwrap();
    let app = server::app::build_app(db_path).unwrap();

    let patch_tag_definition = Request::builder()
        .method("PATCH")
        .uri("/tag-definitions/999")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "value_type": "text" }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone()
            .oneshot(patch_tag_definition)
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    for (method, uri) in [
        ("DELETE", "/tag-definitions/999"),
        ("DELETE", "/tag-enum-options/999"),
        ("DELETE", "/external-entity-types/999"),
        ("DELETE", "/external-entities/999"),
        ("PATCH", "/tag-enum-options/999/retire"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{method} {uri}");
    }
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn insert_category(
    conn: &Connection,
    id: i64,
    slug: &str,
    name: &str,
    parent_category_id: Option<i64>,
) {
    conn.execute(
        "INSERT INTO categories (id, slug, name, parent_category_id) VALUES (?1, ?2, ?3, ?4)",
        (id, slug, name, parent_category_id),
    )
    .unwrap();
}

fn seed_tag_definition_reference_graph(conn: &Connection) {
    conn.execute(
        "INSERT INTO categories (id, slug, name) VALUES (?1, ?2, ?3)",
        (1_i64, "network", "Network"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag) VALUES (?1, ?2, ?3)",
        (1_i64, 1_i64, "AST-1"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO event_types (event_type_id, display_name) VALUES (?1, ?2)",
        ("asset.update", "Asset Update"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (1_i64, "owner", "Owner", "text"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO event_type_mutations (event_type_id, event_type_version, mutation_index, operation, tag_definition_id, input_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        ("asset.update", 1_i64, 0_i64, "set", 1_i64, "owner"),
    )
    .unwrap();
}

fn seed_external_entity_reference_graph(conn: &Connection) {
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        (1_i64, "vendor", "Vendor"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type, external_entity_type_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        (1_i64, "vendor_id", "Vendor", "external_entity", 1_i64),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO external_entities (id, external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3, ?4)",
        (1_i64, 1_i64, "v-1", "Acme"),
    )
    .unwrap();

    conn.execute(
        "INSERT INTO categories (id, slug, name) VALUES (?1, ?2, ?3)",
        (100_i64, "network", "Network"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag) VALUES (?1, ?2, ?3)",
        (100_i64, 100_i64, "AST-100"),
    )
    .unwrap();

    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (2_i64, "status", "Status", "enum"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_enum_options (id, tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (1_i64, 2_i64, "active", "Active", 0_i64, 1_i64),
    )
    .unwrap();

    conn.execute(
        "INSERT INTO asset_current_tag_values (asset_id, tag_definition_id, value_json, enum_option_id, external_entity_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        (100_i64, 2_i64, "\"active\"", 1_i64, 1_i64),
    )
    .unwrap();
}

fn seed_tag_definition_non_event_references(conn: &Connection) {
    conn.execute(
        "INSERT INTO categories (id, slug, name) VALUES (?1, ?2, ?3)",
        (20_i64, "root", "Root"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag) VALUES (?1, ?2, ?3)",
        (20_i64, 20_i64, "AST-20"),
    )
    .unwrap();

    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (10_i64, "hint_ref", "Hint Ref", "text"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO category_tag_hints (category_id, tag_definition_id, is_required, sort_order) VALUES (?1, ?2, ?3, ?4)",
        (20_i64, 10_i64, 0_i64, 0_i64),
    )
    .unwrap();

    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (11_i64, "enum_ref", "Enum Ref", "enum"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tag_enum_options (id, tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (11_i64, 11_i64, "opt", "Option", 0_i64, 1_i64),
    )
    .unwrap();

    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type) VALUES (?1, ?2, ?3, ?4)",
        (12_i64, "value_ref", "Value Ref", "text"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO asset_current_tag_values (asset_id, tag_definition_id, value_json) VALUES (?1, ?2, ?3)",
        (20_i64, 12_i64, "\"x\""),
    )
    .unwrap();
}
