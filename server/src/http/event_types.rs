use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/event-types", post(create_event_type))
        .route("/event-types/:id", get(get_event_type_current))
        .route(
            "/event-types/:id/versions/:version",
            get(get_event_type_version).delete(delete_event_type_version),
        )
        .route("/event-types/:id/versions", post(create_event_type_version))
}

#[derive(Deserialize)]
struct MutationRequest {
    operation: String,
    tag_definition_id: i64,
    #[serde(default)]
    input_key: Option<String>,
}

#[derive(Deserialize)]
struct CreateEventTypeRequest {
    event_type_id: String,
    display_name: String,
    #[serde(default)]
    description: Option<String>,
    mutations: Vec<MutationRequest>,
}

#[derive(Deserialize)]
struct CreateVersionRequest {
    mutations: Vec<MutationRequest>,
}

#[derive(Serialize)]
struct EventTypeMutationResponse {
    mutation_index: i64,
    operation: String,
    tag_definition_id: i64,
    input_key: Option<String>,
}

#[derive(Serialize)]
struct EventTypeResponse {
    event_type_id: String,
    display_name: String,
    description: Option<String>,
    version: i64,
    mutations: Vec<EventTypeMutationResponse>,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn create_event_type(
    State(state): State<AppState>,
    Json(req): Json<CreateEventTypeRequest>,
) -> impl IntoResponse {
    if req.mutations.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "EVENT_TYPE_MUTATIONS_REQUIRED",
                message: "event type must include at least one mutation".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let mutations = req
        .mutations
        .iter()
        .enumerate()
        .map(|(idx, m)| db::repo_events::NewEventMutation {
            mutation_index: idx as i64,
            operation: m.operation.clone(),
            tag_definition_id: m.tag_definition_id,
            input_key: m.input_key.clone(),
        })
        .collect::<Vec<_>>();

    match db::repo_events::create_event_type_initial_version(
        &mut conn,
        &req.event_type_id,
        &req.display_name,
        req.description.as_deref(),
        &mutations,
    ) {
        Ok(record) => (StatusCode::CREATED, Json(to_response(record))).into_response(),
        Err(db::repo_events::EventTypeCreateError::AlreadyExists) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EVENT_TYPE_ALREADY_EXISTS",
                message: "event type already exists".to_string(),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventTypeCreateError::TagDefinitionMissing(id)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "TAG_DEFINITION_NOT_FOUND",
                message: format!("tag definition `{id}` does not exist"),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventTypeCreateError::UnsupportedMutationOperation(operation)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "UNSUPPORTED_MUTATION_OPERATION",
                message: format!("mutation operation `{operation}` is not supported"),
            }),
        )
            .into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

async fn create_event_type_version(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateVersionRequest>,
) -> impl IntoResponse {
    if req.mutations.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "EVENT_TYPE_MUTATIONS_REQUIRED",
                message: "event type version must include at least one mutation".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let mutations = req
        .mutations
        .iter()
        .enumerate()
        .map(|(idx, m)| db::repo_events::NewEventMutation {
            mutation_index: idx as i64,
            operation: m.operation.clone(),
            tag_definition_id: m.tag_definition_id,
            input_key: m.input_key.clone(),
        })
        .collect::<Vec<_>>();

    match db::repo_events::create_event_type_version(&mut conn, &id, &mutations) {
        Ok(record) => (StatusCode::CREATED, Json(to_response(record))).into_response(),
        Err(db::repo_events::EventTypeVersionCreateError::EventTypeNotFound) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(db::repo_events::EventTypeVersionCreateError::TagDefinitionMissing(tag_id)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "TAG_DEFINITION_NOT_FOUND",
                message: format!("tag definition `{tag_id}` does not exist"),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventTypeVersionCreateError::UnsupportedMutationOperation(
            operation,
        )) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "UNSUPPORTED_MUTATION_OPERATION",
                message: format!("mutation operation `{operation}` is not supported"),
            }),
        )
            .into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

async fn get_event_type_current(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    match db::repo_events::load_event_type_current(&conn, &id) {
        Ok(Some(record)) => (StatusCode::OK, Json(to_response(record))).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

async fn get_event_type_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, i64)>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    match db::repo_events::load_event_type_version(&conn, &id, version) {
        Ok(Some(record)) => (StatusCode::OK, Json(to_response(record))).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

async fn delete_event_type_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, i64)>,
) -> impl IntoResponse {
    let mut conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    match db::repo_events::delete_event_type_version(&mut conn, &id, version) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(db::repo_events::EventTypeDeleteVersionError::EventTypeNotFound)
        | Err(db::repo_events::EventTypeDeleteVersionError::VersionNotFound) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(db::repo_events::EventTypeDeleteVersionError::VersionInUse) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EVENT_TYPE_VERSION_IN_USE",
                message: "cannot delete event type version while referenced".to_string(),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventTypeDeleteVersionError::CannotDeleteOnlyVersion) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EVENT_TYPE_VERSION_IN_USE",
                message: "cannot delete only remaining version".to_string(),
            }),
        )
            .into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

fn to_response(record: db::repo_events::EventTypeVersionRecord) -> EventTypeResponse {
    EventTypeResponse {
        event_type_id: record.event_type_id,
        display_name: record.display_name,
        description: record.description,
        version: record.version,
        mutations: record
            .mutations
            .into_iter()
            .map(|m| EventTypeMutationResponse {
                mutation_index: m.mutation_index,
                operation: m.operation,
                tag_definition_id: m.tag_definition_id,
                input_key: m.input_key,
            })
            .collect(),
    }
}

fn internal_error(message: String) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody {
            reason_code: "INTERNAL_ERROR",
            message,
        }),
    )
        .into_response()
}
