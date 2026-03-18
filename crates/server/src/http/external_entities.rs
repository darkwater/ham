use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, patch, post},
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/external-entity-types", post(create_external_entity_type))
        .route(
            "/external-entity-types/:id",
            delete(delete_external_entity_type),
        )
        .route("/external-entities", post(create_external_entity))
        .route("/external-entities/:id", delete(delete_external_entity))
        .route("/tag-enum-options/:id/retire", patch(retire_enum_option))
}

#[derive(Deserialize)]
struct CreateExternalEntityTypeRequest {
    type_key: String,
    display_name: String,
}

#[derive(Serialize)]
struct ExternalEntityTypeResponse {
    id: i64,
    type_key: String,
    display_name: String,
}

#[derive(Deserialize)]
struct CreateExternalEntityRequest {
    external_entity_type_id: i64,
    external_key: String,
    display_name: String,
}

#[derive(Serialize)]
struct ExternalEntityResponse {
    id: i64,
    external_entity_type_id: i64,
    external_key: String,
    display_name: String,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn create_external_entity_type(
    State(state): State<AppState>,
    Json(req): Json<CreateExternalEntityTypeRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let inserted = conn.execute(
        "INSERT INTO external_entity_types (type_key, display_name) VALUES (?1, ?2)",
        rusqlite::params![req.type_key, req.display_name],
    );
    if let Err(err) = inserted {
        if let rusqlite::Error::SqliteFailure(code, _) = &err {
            if code.code == rusqlite::ErrorCode::ConstraintViolation {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorBody {
                        reason_code: "EXTERNAL_ENTITY_TYPE_CREATE_CONFLICT",
                        message: "external entity type create constraint violation".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        return internal_error(err.to_string());
    }

    (
        StatusCode::CREATED,
        Json(ExternalEntityTypeResponse {
            id: conn.last_insert_rowid(),
            type_key: req.type_key,
            display_name: req.display_name,
        }),
    )
        .into_response()
}

async fn create_external_entity(
    State(state): State<AppState>,
    Json(req): Json<CreateExternalEntityRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let inserted = conn.execute(
        "INSERT INTO external_entities (external_entity_type_id, external_key, display_name) VALUES (?1, ?2, ?3)",
        rusqlite::params![req.external_entity_type_id, req.external_key, req.display_name],
    );
    if let Err(err) = inserted {
        if let rusqlite::Error::SqliteFailure(code, _) = &err {
            if code.code == rusqlite::ErrorCode::ConstraintViolation {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorBody {
                        reason_code: "EXTERNAL_ENTITY_CREATE_CONFLICT",
                        message: "external entity create constraint violation".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        return internal_error(err.to_string());
    }

    (
        StatusCode::CREATED,
        Json(ExternalEntityResponse {
            id: conn.last_insert_rowid(),
            external_entity_type_id: req.external_entity_type_id,
            external_key: req.external_key,
            display_name: req.display_name,
        }),
    )
        .into_response()
}

async fn delete_external_entity_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row(
            "SELECT 1 FROM external_entity_types WHERE id = ?1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM tag_definitions WHERE external_entity_type_id = ?1 LIMIT 1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if referenced {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EXTERNAL_ENTITY_TYPE_IN_USE",
                message: "cannot delete external entity type while referenced".to_string(),
            }),
        )
            .into_response();
    }

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM external_entities WHERE external_entity_type_id = ?1 LIMIT 1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if referenced {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EXTERNAL_ENTITY_TYPE_IN_USE",
                message: "cannot delete external entity type while referenced".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(err) = conn.execute("DELETE FROM external_entity_types WHERE id = ?1", [id]) {
        return internal_error(err.to_string());
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn delete_external_entity(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row(
            "SELECT 1 FROM external_entities WHERE id = ?1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM asset_current_tag_values WHERE external_entity_id = ?1 LIMIT 1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if referenced {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "EXTERNAL_ENTITY_IN_USE",
                message: "cannot hard-delete external entity while referenced".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(err) = conn.execute("DELETE FROM external_entities WHERE id = ?1", [id]) {
        return internal_error(err.to_string());
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn retire_enum_option(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row(
            "SELECT 1 FROM tag_enum_options WHERE id = ?1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Err(err) = conn.execute(
        "UPDATE tag_enum_options SET is_active = 0 WHERE id = ?1",
        [id],
    ) {
        return internal_error(err.to_string());
    }
    StatusCode::NO_CONTENT.into_response()
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
