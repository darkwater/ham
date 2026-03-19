use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

#[tokio::test]
async fn scripted_core_flow_json_output_is_automation_friendly() {
    let server = StubServer::start(StubConfig::default()).await;

    let out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "flow",
        "scripted-core",
    ])
    .await;

    assert!(out.status.success(), "stderr: {}", out.stderr);

    let body: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["flow"], "scripted-core");

    let steps = body["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 12);
    assert_eq!(steps[0]["action"], "create_category");
    assert_eq!(steps[1]["action"], "create_tag_definition_text");
    assert_eq!(steps[2]["action"], "create_tag_definition_enum");
    assert_eq!(steps[3]["action"], "create_enum_option");
    assert_eq!(steps[4]["action"], "create_external_entity_type");
    assert_eq!(steps[5]["action"], "create_external_entity");
    assert_eq!(steps[6]["action"], "create_tag_definition_external_entity");
    assert_eq!(steps[7]["action"], "create_event_type");
    assert_eq!(steps[8]["action"], "create_asset");
    assert_eq!(steps[9]["action"], "apply_event");
    assert_eq!(steps[10]["action"], "fetch_timeline");
    assert_eq!(steps[11]["action"], "run_search");
}

#[tokio::test]
async fn scripted_core_flow_uses_created_category_id_and_idempotency_key() {
    let server = StubServer::start(StubConfig {
        category_id: 9876,
        fail_on_wrong_asset_category: true,
        require_idempotency_key: true,
        fail_on_external_entity_type_create: false,
    })
    .await;

    let out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "flow",
        "scripted-core",
    ])
    .await;

    assert!(out.status.success(), "stderr: {}", out.stderr);
    let guard = server.state.lock().unwrap();
    assert_eq!(guard.last_asset_category_id, Some(9876));
    assert_eq!(guard.last_idempotency_key.as_deref(), Some("ham-flow-001"));
}

#[tokio::test]
async fn scripted_core_flow_surfaces_stable_error_shape() {
    let server = StubServer::start(StubConfig {
        fail_on_external_entity_type_create: true,
        ..StubConfig::default()
    })
    .await;

    let out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "flow",
        "scripted-core",
    ])
    .await;

    assert!(!out.status.success());
    let body: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "HTTP_ERROR");
    assert_eq!(body["error"]["step"], "create_external_entity_type");
    assert!(body["error"]["status_code"].as_i64().unwrap() >= 400);
}

#[tokio::test]
async fn category_create_http_error_uses_command_error_envelope() {
    let server = StubServer::start(StubConfig::default()).await;

    let out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "invalid",
    ])
    .await;

    assert!(!out.status.success());

    let body: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(body["ok"], false);
    assert_eq!(body["command"], "category create");
    assert_eq!(body["error"]["code"], "HTTP_ERROR");
    assert_eq!(body["error"]["step"], "category_create");
    assert_eq!(body["error"]["status_code"], 400);
}

#[tokio::test]
async fn category_create_posts_expected_payload() {
    let server = StubServer::start(StubConfig::default()).await;

    let root_out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "Network",
    ])
    .await;
    assert!(root_out.status.success(), "stderr: {}", root_out.stderr);

    let child_out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "Child",
        "--parent-id",
        "10",
    ])
    .await;
    assert!(child_out.status.success(), "stderr: {}", child_out.stderr);

    let guard = server.state.lock().unwrap();
    assert_eq!(
        guard.category_create_payloads,
        vec![
            json!({"name":"Network"}),
            json!({"name":"Child","parent_category_id":10})
        ]
    );
}

#[tokio::test]
async fn category_create_accepts_contract_valid_payload_shape() {
    let server = StubServer::start(StubConfig::default()).await;

    let out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "Telemetry",
    ])
    .await;

    assert!(out.status.success(), "stderr: {}", out.stderr);

    let guard = server.state.lock().unwrap();
    assert_eq!(guard.category_create_payloads, vec![json!({"name":"Telemetry"})]);
}

#[tokio::test]
async fn create_category_stub_rejects_non_contract_payload_shape() {
    let state = Arc::new(Mutex::new(SharedState {
        config: Some(StubConfig::default()),
        ..SharedState::default()
    }));

    let (status, _) = create_category(
        State(state.clone()),
        Json(json!({"name":"Telemetry","parent_category_id":"10"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = create_category(State(state), Json(json!({"name":"Telemetry","extra":true}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn category_create_blank_name_returns_validation_error() {
    let server = StubServer::start(StubConfig::default()).await;

    let out = run_cli([
        "--base-url",
        &server.base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "   ",
    ])
    .await;

    assert!(!out.status.success());

    let body: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    assert_eq!(body["error"]["step"], "category_create");
}

#[tokio::test]
async fn category_create_and_list_succeeds_against_real_server_app() {
    let db_file = tempfile::NamedTempFile::new().unwrap();
    let app = server::app::build_app(db_file.path().to_path_buf()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let _join = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let create_out = run_cli([
        "--base-url",
        &base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "Network",
    ])
    .await;
    assert!(
        create_out.status.success(),
        "stderr: {}\nstdout: {}",
        create_out.stderr,
        create_out.stdout
    );

    let list_out = run_cli(["--base-url", &base_url, "--output", "json", "category", "list"]).await;
    assert!(
        list_out.status.success(),
        "stderr: {}\nstdout: {}",
        list_out.stderr,
        list_out.stdout
    );

    let list_body: Value = serde_json::from_str(&list_out.stdout).unwrap();
    assert_eq!(list_body["ok"], true);
    assert_eq!(list_body["command"], "category list");
    assert_eq!(list_body["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(list_body["result"]["items"][0]["name"], "Network");
}

#[tokio::test]
async fn category_create_and_delete_succeeds_against_real_server_app() {
    let db_file = tempfile::NamedTempFile::new().unwrap();
    let app = server::app::build_app(db_file.path().to_path_buf()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let _join = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let create_out = run_cli([
        "--base-url",
        &base_url,
        "--output",
        "json",
        "category",
        "create",
        "--name",
        "Delete Me",
    ])
    .await;
    assert!(
        create_out.status.success(),
        "stderr: {}\nstdout: {}",
        create_out.stderr,
        create_out.stdout
    );

    let create_body: Value = serde_json::from_str(&create_out.stdout).unwrap();
    let category_id = create_body["result"]["id"].as_i64().unwrap();

    let delete_out = run_cli([
        "--base-url",
        &base_url,
        "--output",
        "json",
        "category",
        "delete",
        "--id",
        &category_id.to_string(),
    ])
    .await;
    assert!(
        delete_out.status.success(),
        "stderr: {}\nstdout: {}",
        delete_out.stderr,
        delete_out.stdout
    );

    let delete_body: Value = serde_json::from_str(&delete_out.stdout).unwrap();
    assert_eq!(delete_body["ok"], true);
    assert_eq!(delete_body["command"], "category delete");
    assert!(delete_body["result"].is_null());

    let list_out = run_cli(["--base-url", &base_url, "--output", "json", "category", "list"]).await;
    assert!(
        list_out.status.success(),
        "stderr: {}\nstdout: {}",
        list_out.stderr,
        list_out.stdout
    );

    let list_body: Value = serde_json::from_str(&list_out.stdout).unwrap();
    assert_eq!(list_body["result"]["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn scripted_core_flow_succeeds_against_real_server_app() {
    let db_file = tempfile::NamedTempFile::new().unwrap();
    let app = server::app::build_app(db_file.path().to_path_buf()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let _join = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let out = run_cli([
        "--base-url",
        &base_url,
        "--output",
        "json",
        "flow",
        "scripted-core",
    ])
    .await;

    assert!(
        out.status.success(),
        "stderr: {}\nstdout: {}",
        out.stderr,
        out.stdout
    );

    let body: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(body["ok"], true);
    let apply_event = body["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "apply_event")
        .unwrap();
    assert_eq!(apply_event["status_code"], 201);
}

#[derive(Clone, Copy)]
struct StubConfig {
    category_id: i64,
    fail_on_wrong_asset_category: bool,
    require_idempotency_key: bool,
    fail_on_external_entity_type_create: bool,
}

impl Default for StubConfig {
    fn default() -> Self {
        Self {
            category_id: 1000,
            fail_on_wrong_asset_category: false,
            require_idempotency_key: false,
            fail_on_external_entity_type_create: false,
        }
    }
}

#[derive(Default)]
struct SharedState {
    config: Option<StubConfig>,
    last_asset_category_id: Option<i64>,
    last_idempotency_key: Option<String>,
    category_create_payloads: Vec<Value>,
}

struct StubServer {
    base_url: String,
    state: Arc<Mutex<SharedState>>,
    _join: tokio::task::JoinHandle<()>,
}

impl StubServer {
    async fn start(config: StubConfig) -> Self {
        let state = Arc::new(Mutex::new(SharedState {
            config: Some(config),
            ..SharedState::default()
        }));

        let app = Router::new()
            .route("/categories", post(create_category))
            .route("/tag-definitions", post(create_tag_definition))
            .route("/tag-enum-options", post(create_enum_option))
            .route("/external-entity-types", post(create_external_entity_type))
            .route("/external-entities", post(create_external_entity))
            .route("/event-types", post(create_event_type))
            .route("/assets", get(assets_list).post(create_asset))
            .route(
                "/assets/:asset_tag/events",
                get(list_timeline).post(apply_event),
            )
            .route("/assets/search", post(run_search))
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let join = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url,
            state,
            _join: join,
        }
    }
}

async fn create_category(
    State(state): State<Arc<Mutex<SharedState>>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let mut guard = state.lock().unwrap();
    guard.category_create_payloads.push(payload.clone());

    let payload_obj = match payload.as_object() {
        Some(payload_obj) => payload_obj,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"reason_code":"INVALID_CATEGORY_PAYLOAD"})),
            )
        }
    };

    let name = match payload_obj.get("name").and_then(Value::as_str) {
        Some(name) if !name.trim().is_empty() => name,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"reason_code":"INVALID_CATEGORY_PAYLOAD"})),
            )
        }
    };

    let parent_category_id = match payload_obj.get("parent_category_id") {
        Some(value) => match value.as_i64() {
            Some(parent_category_id) => Some(parent_category_id),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"reason_code":"INVALID_CATEGORY_PAYLOAD"})),
                )
            }
        },
        None => None,
    };

    if payload_obj
        .keys()
        .any(|key| key != "name" && key != "parent_category_id")
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_CATEGORY_PAYLOAD"})),
        );
    }

    if name == "invalid" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_CATEGORY_PAYLOAD"})),
        );
    }

    let category_id = guard.config.unwrap().category_id;
    (
        StatusCode::CREATED,
        Json(json!({"id":category_id,"name":name,"parent_category_id":parent_category_id})),
    )
}

async fn create_tag_definition(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
    let value_type = payload
        .get("value_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if value_type.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_TAG_DEFINITION_PAYLOAD"})),
        );
    }
    let id = match value_type {
        "text" => 2001,
        "enum" => 2002,
        "external_entity" => 2003,
        _ => 2999,
    };
    (
        StatusCode::CREATED,
        Json(json!({"id":id,"value_type":value_type})),
    )
}

async fn create_enum_option(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
    if payload
        .get("tag_definition_id")
        .and_then(Value::as_i64)
        .is_none()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_ENUM_OPTION_PAYLOAD"})),
        );
    }
    (StatusCode::CREATED, Json(json!({"id":5001})))
}

async fn create_external_entity_type(
    State(state): State<Arc<Mutex<SharedState>>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if state
        .lock()
        .unwrap()
        .config
        .unwrap()
        .fail_on_external_entity_type_create
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID"})),
        );
    }
    if payload.get("type_key").and_then(Value::as_str) != Some("vendor") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_EXTERNAL_ENTITY_TYPE_PAYLOAD"})),
        );
    }
    (StatusCode::CREATED, Json(json!({"id":3001})))
}

async fn create_external_entity(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
    if payload
        .get("external_entity_type_id")
        .and_then(Value::as_i64)
        .is_none()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_EXTERNAL_ENTITY_PAYLOAD"})),
        );
    }
    (StatusCode::CREATED, Json(json!({"id":4001})))
}

async fn create_event_type(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
    if payload.get("event_type_id").and_then(Value::as_str) != Some("asset.set-owner") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_EVENT_TYPE_PAYLOAD"})),
        );
    }
    (
        StatusCode::CREATED,
        Json(json!({"event_type_id":"asset.set-owner","version":1})),
    )
}

async fn create_asset(
    State(state): State<Arc<Mutex<SharedState>>>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let category_id = payload
        .get("category_id")
        .and_then(Value::as_i64)
        .unwrap_or_default();

    let mut guard = state.lock().unwrap();
    guard.last_asset_category_id = Some(category_id);

    if guard.config.unwrap().fail_on_wrong_asset_category
        && category_id != guard.config.unwrap().category_id
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_CATEGORY_ID"})),
        );
    }

    (
        StatusCode::CREATED,
        Json(json!({"id":1,"asset_tag":"AST-FLOW-001"})),
    )
}

async fn apply_event(
    State(state): State<Arc<Mutex<SharedState>>>,
    Path(_asset_tag): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if payload.get("event_type_id").and_then(Value::as_str) != Some("asset.set-owner") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_APPLY_EVENT_PAYLOAD"})),
        );
    }
    if payload
        .pointer("/payload/owner")
        .and_then(Value::as_str)
        .is_none()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_APPLY_EVENT_PAYLOAD"})),
        );
    }
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let mut guard = state.lock().unwrap();
    guard.last_idempotency_key = key.clone();
    if guard.config.unwrap().require_idempotency_key && key.as_deref() != Some("ham-flow-001") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"MISSING_IDEMPOTENCY_KEY"})),
        );
    }

    (StatusCode::CREATED, Json(json!({"event_id":1})))
}

async fn list_timeline() -> Json<Value> {
    Json(json!({"items":[{"event_id":1}],"next_cursor":null}))
}

async fn run_search(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
    if payload.get("filters").and_then(Value::as_array).is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"reason_code":"INVALID_SEARCH_PAYLOAD"})),
        );
    }
    (
        StatusCode::OK,
        Json(json!({"items":[{"id":1,"asset_tag":"AST-FLOW-001"}],"next_cursor":null})),
    )
}

async fn assets_list() -> Json<Value> {
    Json(json!({"items":[]}))
}

struct CmdOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

async fn run_cli<'a>(args: impl IntoIterator<Item = &'a str>) -> CmdOutput {
    let mut cmd_args = vec![
        OsString::from("run"),
        OsString::from("-p"),
        OsString::from("cli"),
        OsString::from("--quiet"),
        OsString::from("--"),
    ];
    cmd_args.extend(args.into_iter().map(OsString::from));

    let output = tokio::task::spawn_blocking(move || {
        Command::new("cargo")
            .args(cmd_args)
            .current_dir(workspace_root())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    CmdOutput {
        status: output.status,
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
