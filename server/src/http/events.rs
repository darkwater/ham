use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/assets/:asset_tag/events",
        post(create_event).get(list_events),
    )
}

#[derive(Deserialize)]
struct CreateEventRequest {
    event_type_id: String,
    #[serde(default)]
    timestamp: Option<String>,
    payload: Value,
}

#[derive(Serialize)]
struct CreateEventResponse {
    event_id: i64,
    asset_id: i64,
    event_type_id: String,
    event_type_version: i64,
    payload: Value,
    timestamp: String,
    replayed: bool,
}

#[derive(Deserialize)]
struct EventListQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Serialize)]
struct EventListItemResponse {
    event_id: i64,
    event_type_id: String,
    event_type_version: i64,
    payload: Value,
    timestamp: String,
    idempotency_key: String,
}

#[derive(Serialize)]
struct EventListResponse {
    items: Vec<EventListItemResponse>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn create_event(
    State(state): State<AppState>,
    Path(asset_tag): Path<String>,
    headers: HeaderMap,
    Json(req): Json<CreateEventRequest>,
) -> impl IntoResponse {
    let idempotency_key = match headers
        .get("Idempotency-Key")
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
    {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody {
                    reason_code: "IDEMPOTENCY_KEY_REQUIRED",
                    message: "Idempotency-Key header is required".to_string(),
                }),
            )
                .into_response()
        }
    };

    if req.timestamp.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "EVENT_TIMESTAMP_FORBIDDEN",
                message: "client timestamp override is not allowed".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    match db::repo_events::apply_asset_event(
        &mut conn,
        &asset_tag,
        &idempotency_key,
        &req.event_type_id,
        req.payload,
    ) {
        Ok(applied) => {
            let status = if applied.replayed {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            };
            (
                status,
                Json(CreateEventResponse {
                    event_id: applied.event_id,
                    asset_id: applied.asset_id,
                    event_type_id: applied.event_type_id,
                    event_type_version: applied.event_type_version,
                    payload: applied.payload,
                    timestamp: applied.created_at,
                    replayed: applied.replayed,
                }),
            )
                .into_response()
        }
        Err(db::repo_events::EventApplyRepoError::AssetNotFound)
        | Err(db::repo_events::EventApplyRepoError::EventTypeNotFound) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(db::repo_events::EventApplyRepoError::IdempotencyPayloadMismatch) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH",
                message: "same idempotency key used with different payload".to_string(),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventApplyRepoError::EventTypeMutationInvalid(message)) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EVENT_TYPE_MUTATION_INVALID",
                message,
            }),
        )
            .into_response(),
        Err(db::repo_events::EventApplyRepoError::ExternalEntityNotFound { .. }) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "INVALID_EVENT_PAYLOAD",
                message: "external entity reference does not exist".to_string(),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventApplyRepoError::EnumOptionNotFound { .. }) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "INVALID_EVENT_PAYLOAD",
                message: "enum option reference does not exist".to_string(),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventApplyRepoError::ExternalEntityTypeMismatch { .. })
        | Err(db::repo_events::EventApplyRepoError::ExternalEntityTypeMissing(_)) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EVENT_VALUE_REFERENCE_CONFLICT",
                message: "event value reference type mismatch".to_string(),
            }),
        )
            .into_response(),
        Err(db::repo_events::EventApplyRepoError::InvalidPayload(message)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "INVALID_EVENT_PAYLOAD",
                message,
            }),
        )
            .into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

async fn list_events(
    State(state): State<AppState>,
    Path(asset_tag): Path<String>,
    Query(query): Query<EventListQuery>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let limit = query.limit.clamp(1, 100);

    match db::repo_events::list_asset_events(&conn, &asset_tag, limit, query.cursor.as_deref()) {
        Ok(page) => (
            StatusCode::OK,
            Json(EventListResponse {
                items: page
                    .items
                    .into_iter()
                    .map(|item| EventListItemResponse {
                        event_id: item.event_id,
                        event_type_id: item.event_type_id,
                        event_type_version: item.event_type_version,
                        payload: item.payload,
                        timestamp: item.timestamp,
                        idempotency_key: item.idempotency_key,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            }),
        )
            .into_response(),
        Err(db::repo_events::EventListError::AssetNotFound) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(db::repo_events::EventListError::InvalidCursor) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "INVALID_CURSOR",
                message: "cursor format is invalid".to_string(),
            }),
        )
            .into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

fn default_limit() -> usize {
    50
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
