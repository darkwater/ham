use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn assets_search_defaults_to_stable_asset_tag_sort_with_cursor() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("search-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_category(&conn, 1, "Network", None);
    seed_asset(&conn, 1, 1, "AST-200", Some("Core"));
    seed_asset(&conn, 2, 1, "AST-100", Some("Edge"));
    seed_asset(&conn, 3, 1, "AST-300", Some("Spare"));

    let app = server::app::build_app(db_path).unwrap();

    let first_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "limit": 2 }).to_string()))
        .unwrap();
    let first_res = app.clone().oneshot(first_req).await.unwrap();
    assert_eq!(first_res.status(), StatusCode::OK);
    let first_body = read_json(first_res.into_body()).await;

    assert_eq!(first_body["items"].as_array().unwrap().len(), 2);
    assert_eq!(first_body["items"][0]["asset_tag"], "AST-100");
    assert_eq!(first_body["items"][1]["asset_tag"], "AST-200");
    let cursor = first_body["next_cursor"].as_str().unwrap().to_string();

    let second_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "limit": 2, "cursor": cursor }).to_string(),
        ))
        .unwrap();
    let second_res = app.oneshot(second_req).await.unwrap();
    assert_eq!(second_res.status(), StatusCode::OK);
    let second_body = read_json(second_res.into_body()).await;

    assert_eq!(second_body["items"].as_array().unwrap().len(), 1);
    assert_eq!(second_body["items"][0]["asset_tag"], "AST-300");
    assert!(second_body["next_cursor"].is_null());
}

#[tokio::test]
async fn assets_search_supports_and_filters_or_groups_and_category_subtree_with_text_predicate() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("search-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_category(&conn, 1, "Root", None);
    seed_category(&conn, 2, "Network", Some(1));
    seed_category(&conn, 3, "Other", None);

    seed_asset(&conn, 10, 2, "AST-NET-001", Some("Edge router"));
    seed_asset(&conn, 11, 2, "AST-NET-002", Some("Access switch"));
    seed_asset(&conn, 12, 3, "AST-OTH-001", Some("Edge router"));

    let app = server::app::build_app(db_path).unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "filters": [
                    {
                        "field": "category_id",
                        "op": "eq",
                        "value": 1,
                        "include_subtree": true
                    }
                ],
                "or_groups": [
                    {
                        "filters": [
                            { "field": "asset_tag", "op": "eq", "value": "AST-NET-002" }
                        ]
                    },
                    {
                        "filters": [
                            { "field": "text", "op": "contains", "value": "router" }
                        ]
                    }
                ],
                "sort": [
                    { "field": "asset_tag", "direction": "asc" }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    let items = body["items"].as_array().unwrap();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["asset_tag"], "AST-NET-001");
    assert_eq!(items[1]["asset_tag"], "AST-NET-002");
}

#[tokio::test]
async fn assets_search_supports_type_specific_operators_and_null_semantics() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("search-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_category(&conn, 1, "Network", None);
    seed_asset(&conn, 100, 1, "AST-TYP-001", Some("Firewall"));
    seed_asset(&conn, 101, 1, "AST-TYP-002", Some("Switch"));

    seed_tag_definition(&conn, 1, "cost", "Cost", "money", None);
    seed_tag_definition(&conn, 2, "is_critical", "Critical", "boolean", None);
    seed_tag_definition(&conn, 3, "purchase_date", "Purchase Date", "date", None);
    seed_tag_definition(&conn, 4, "status", "Status", "enum", None);
    seed_external_entity_type(&conn, 10, "vendor", "Vendor");
    seed_tag_definition(
        &conn,
        5,
        "owner_entity",
        "Owner Entity",
        "external_entity",
        Some(10),
    );
    seed_tag_definition(&conn, 6, "ip_address", "IP", "ipv4", None);

    seed_enum_option(&conn, 1, 4, "active", "Active", 0);
    seed_enum_option(&conn, 2, 4, "retired", "Retired", 1);
    seed_external_entity(&conn, 7, 10, "v-7", "Contoso");

    seed_tag_value(&conn, 100, 1, "\"100.50\"", None, None);
    seed_tag_value(&conn, 100, 2, "true", None, None);
    seed_tag_value(&conn, 100, 3, "\"2024-01-10\"", None, None);
    seed_tag_value(&conn, 100, 4, "\"active\"", Some(1), None);
    seed_tag_value(&conn, 100, 5, "7", None, Some(7));
    seed_tag_value(&conn, 100, 6, "\"10.0.0.1\"", None, None);

    seed_tag_value(&conn, 101, 1, "\"50.00\"", None, None);
    seed_tag_value(&conn, 101, 2, "false", None, None);
    seed_tag_value(&conn, 101, 4, "\"retired\"", Some(2), None);
    seed_tag_value(&conn, 101, 6, "\"10.0.0.2\"", None, None);

    let app = server::app::build_app(db_path.clone()).unwrap();

    let strict_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "filters": [
                    { "field": "cost", "op": "gt", "value": 60 },
                    { "field": "is_critical", "op": "eq", "value": true },
                    { "field": "purchase_date", "op": "between", "values": ["2024-01-01", "2024-12-31"] },
                    { "field": "status", "op": "eq", "value": "active" },
                    { "field": "external_entity(10)", "op": "eq", "value": 7 },
                    { "field": "ip_address", "op": "eq", "value": "10.0.0.1" }
                ],
                "include_total_estimate": true
            })
            .to_string(),
        ))
        .unwrap();
    let strict_res = app.clone().oneshot(strict_req).await.unwrap();
    assert_eq!(strict_res.status(), StatusCode::OK);
    let strict_body = read_json(strict_res.into_body()).await;
    assert_eq!(strict_body["items"].as_array().unwrap().len(), 1);
    assert_eq!(strict_body["items"][0]["asset_tag"], "AST-TYP-001");
    assert_eq!(strict_body["total_estimate"], 1);

    let null_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "filters": [
                    { "field": "purchase_date", "op": "is_null" }
                ],
                "sort": [
                    { "field": "asset_tag", "direction": "asc" }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let null_res = app.oneshot(null_req).await.unwrap();
    assert_eq!(null_res.status(), StatusCode::OK);
    let null_body = read_json(null_res.into_body()).await;
    assert_eq!(null_body["items"].as_array().unwrap().len(), 1);
    assert_eq!(null_body["items"][0]["asset_tag"], "AST-TYP-002");
}

#[tokio::test]
async fn assets_search_numeric_tag_sort_is_numeric_not_lexicographic() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("search-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_category(&conn, 1, "Network", None);
    seed_asset(&conn, 201, 1, "AST-SORT-201", Some("A"));
    seed_asset(&conn, 202, 1, "AST-SORT-202", Some("B"));
    seed_asset(&conn, 203, 1, "AST-SORT-203", Some("C"));

    seed_tag_definition(&conn, 21, "cost", "Cost", "money", None);
    seed_tag_value(&conn, 201, 21, "\"2\"", None, None);
    seed_tag_value(&conn, 202, 21, "\"10\"", None, None);
    seed_tag_value(&conn, 203, 21, "\"100\"", None, None);

    let app = server::app::build_app(db_path).unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "sort": [
                    { "field": "cost", "direction": "asc" }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    let items = body["items"].as_array().unwrap();

    assert_eq!(items[0]["asset_tag"], "AST-SORT-201");
    assert_eq!(items[1]["asset_tag"], "AST-SORT-202");
    assert_eq!(items[2]["asset_tag"], "AST-SORT-203");
}

#[tokio::test]
async fn assets_search_returns_400_for_invalid_contract_inputs() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("search-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_category(&conn, 1, "Network", None);
    seed_asset(&conn, 301, 1, "AST-ERR-301", Some("Err"));
    seed_tag_definition(&conn, 31, "is_critical", "Critical", "boolean", None);
    seed_tag_value(&conn, 301, 31, "true", None, None);

    let app = server::app::build_app(db_path).unwrap();

    let bad_sort_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "sort": [
                    { "field": "asset_tag", "direction": "upward" }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let bad_sort_res = app.clone().oneshot(bad_sort_req).await.unwrap();
    assert_eq!(bad_sort_res.status(), StatusCode::BAD_REQUEST);
    let bad_sort_body = read_json(bad_sort_res.into_body()).await;
    assert_eq!(bad_sort_body["reason_code"], "INVALID_SEARCH_REQUEST");

    let bad_op_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "filters": [
                    { "field": "is_critical", "op": "contains", "value": true }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let bad_op_res = app.clone().oneshot(bad_op_req).await.unwrap();
    assert_eq!(bad_op_res.status(), StatusCode::BAD_REQUEST);
    let bad_op_body = read_json(bad_op_res.into_body()).await;
    assert_eq!(bad_op_body["reason_code"], "INVALID_SEARCH_REQUEST");

    let bad_cursor_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "cursor": "not-a-number" }).to_string()))
        .unwrap();
    let bad_cursor_res = app.clone().oneshot(bad_cursor_req).await.unwrap();
    assert_eq!(bad_cursor_res.status(), StatusCode::BAD_REQUEST);
    let bad_cursor_body = read_json(bad_cursor_res.into_body()).await;
    assert_eq!(bad_cursor_body["reason_code"], "INVALID_SEARCH_REQUEST");

    let bad_value_req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "filters": [
                    { "field": "category_id", "op": "eq", "value": "wrong-type" }
                ]
            })
            .to_string(),
        ))
        .unwrap();
    let bad_value_res = app.oneshot(bad_value_req).await.unwrap();
    assert_eq!(bad_value_res.status(), StatusCode::BAD_REQUEST);
    let bad_value_body = read_json(bad_value_res.into_body()).await;
    assert_eq!(bad_value_body["reason_code"], "INVALID_SEARCH_REQUEST");
}

#[tokio::test]
async fn assets_search_includes_event_applied_enum_and_external_entity_values() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("search-http.db");
    let conn = server::db::open_and_prepare(&db_path).unwrap();

    seed_category(&conn, 1, "Network", None);
    seed_asset(&conn, 401, 1, "AST-EVT-401", Some("Router"));
    seed_asset(&conn, 402, 1, "AST-EVT-402", Some("Switch"));

    seed_external_entity_type(&conn, 10, "vendor", "Vendor");
    seed_external_entity(&conn, 71, 10, "v-71", "Vendor 71");

    seed_tag_definition(&conn, 41, "status", "Status", "enum", None);
    seed_tag_definition(
        &conn,
        42,
        "vendor_id",
        "Vendor",
        "external_entity",
        Some(10),
    );
    seed_enum_option(&conn, 51, 41, "active", "Active", 0);

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
                        "tag_definition_id": 41,
                        "input_key": "status"
                    },
                    {
                        "operation": "set",
                        "tag_definition_id": 42,
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
        .uri("/assets/AST-EVT-401/events")
        .header("content-type", "application/json")
        .header("Idempotency-Key", "idem-search")
        .body(Body::from(
            json!({
                "event_type_id": "asset.classify",
                "payload": {
                    "status": "active",
                    "vendor_id": 71
                }
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(apply_event_req).await.unwrap().status(),
        StatusCode::CREATED
    );

    let req = Request::builder()
        .method("POST")
        .uri("/assets/search")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "filters": [
                    { "field": "status", "op": "eq", "value": "active" },
                    { "field": "external_entity(10)", "op": "eq", "value": 71 }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["asset_tag"], "AST-EVT-401");
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn seed_category(conn: &Connection, id: i64, name: &str, parent_category_id: Option<i64>) {
    conn.execute(
        "INSERT INTO categories (id, name, parent_category_id) VALUES (?1, ?2, ?3)",
        params![id, name, parent_category_id],
    )
    .unwrap();
}

fn seed_asset(
    conn: &Connection,
    id: i64,
    category_id: i64,
    asset_tag: &str,
    display_name: Option<&str>,
) {
    conn.execute(
        "INSERT INTO assets (id, category_id, asset_tag, display_name) VALUES (?1, ?2, ?3, ?4)",
        params![id, category_id, asset_tag, display_name],
    )
    .unwrap();
}

fn seed_tag_definition(
    conn: &Connection,
    id: i64,
    tag_key: &str,
    display_name: &str,
    value_type: &str,
    external_entity_type_id: Option<i64>,
) {
    conn.execute(
        "INSERT INTO tag_definitions (id, tag_key, display_name, value_type, external_entity_type_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, tag_key, display_name, value_type, external_entity_type_id],
    )
    .unwrap();
}

fn seed_enum_option(
    conn: &Connection,
    id: i64,
    tag_definition_id: i64,
    option_key: &str,
    display_name: &str,
    sort_order: i64,
) {
    conn.execute(
        "INSERT INTO tag_enum_options (id, tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![id, tag_definition_id, option_key, display_name, sort_order],
    )
    .unwrap();
}

fn seed_external_entity_type(conn: &Connection, id: i64, type_key: &str, display_name: &str) {
    conn.execute(
        "INSERT INTO external_entity_types (id, type_key, display_name) VALUES (?1, ?2, ?3)",
        params![id, type_key, display_name],
    )
    .unwrap();
}

fn seed_external_entity(
    conn: &Connection,
    id: i64,
    external_entity_type_id: i64,
    external_key: &str,
    display_name: &str,
) {
    conn.execute(
        "INSERT INTO external_entities (id, external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3, ?4)",
        params![id, external_entity_type_id, external_key, display_name],
    )
    .unwrap();
}

fn seed_tag_value(
    conn: &Connection,
    asset_id: i64,
    tag_definition_id: i64,
    value_json: &str,
    enum_option_id: Option<i64>,
    external_entity_id: Option<i64>,
) {
    conn.execute(
        "INSERT INTO asset_current_tag_values (asset_id, tag_definition_id, value_json, enum_option_id, external_entity_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![asset_id, tag_definition_id, value_json, enum_option_id, external_entity_id],
    )
    .unwrap();
}
