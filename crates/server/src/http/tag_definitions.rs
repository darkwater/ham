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
        .route("/tag-definitions", post(create_tag_definition))
        .route(
            "/tag-definitions/:id",
            patch(update_tag_definition).delete(delete_tag_definition),
        )
        .route("/tag-enum-options", post(create_tag_enum_option))
        .route("/tag-enum-options/:id", delete(delete_tag_enum_option))
}

#[derive(Deserialize)]
struct CreateTagDefinitionRequest {
    tag_key: String,
    display_name: String,
    value_type: String,
    #[serde(default)]
    external_entity_type_id: Option<i64>,
}

#[derive(Serialize)]
struct TagDefinitionResponse {
    id: i64,
    tag_key: String,
    display_name: String,
    value_type: String,
    external_entity_type_id: Option<i64>,
}

#[derive(Deserialize)]
struct CreateTagEnumOptionRequest {
    tag_definition_id: i64,
    option_key: String,
    display_name: String,
    #[serde(default)]
    sort_order: i64,
}

#[derive(Serialize)]
struct TagEnumOptionResponse {
    id: i64,
    tag_definition_id: i64,
    option_key: String,
    display_name: String,
    sort_order: i64,
    is_active: bool,
}

#[derive(Deserialize)]
struct UpdateTagDefinitionRequest {
    #[serde(default)]
    value_type: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn create_tag_definition(
    State(state): State<AppState>,
    Json(req): Json<CreateTagDefinitionRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let inserted = conn.execute(
        "INSERT INTO tag_definitions (tag_key, display_name, value_type, external_entity_type_id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![req.tag_key, req.display_name, req.value_type, req.external_entity_type_id],
    );

    if let Err(err) = inserted {
        if let rusqlite::Error::SqliteFailure(code, _) = &err {
            if code.code == rusqlite::ErrorCode::ConstraintViolation {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorBody {
                        reason_code: "TAG_DEFINITION_CREATE_CONFLICT",
                        message: "tag definition create constraint violation".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        return internal_error(err.to_string());
    }

    (
        StatusCode::CREATED,
        Json(TagDefinitionResponse {
            id: conn.last_insert_rowid(),
            tag_key: req.tag_key,
            display_name: req.display_name,
            value_type: req.value_type,
            external_entity_type_id: req.external_entity_type_id,
        }),
    )
        .into_response()
}

async fn create_tag_enum_option(
    State(state): State<AppState>,
    Json(req): Json<CreateTagEnumOptionRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let inserted = conn.execute(
        "INSERT INTO tag_enum_options (tag_definition_id, option_key, display_name, sort_order, is_active) VALUES (?1, ?2, ?3, ?4, 1)",
        rusqlite::params![req.tag_definition_id, req.option_key, req.display_name, req.sort_order],
    );

    if let Err(err) = inserted {
        if let rusqlite::Error::SqliteFailure(code, _) = &err {
            if code.code == rusqlite::ErrorCode::ConstraintViolation {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorBody {
                        reason_code: "TAG_ENUM_OPTION_CREATE_CONFLICT",
                        message: "tag enum option create constraint violation".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        return internal_error(err.to_string());
    }

    (
        StatusCode::CREATED,
        Json(TagEnumOptionResponse {
            id: conn.last_insert_rowid(),
            tag_definition_id: req.tag_definition_id,
            option_key: req.option_key,
            display_name: req.display_name,
            sort_order: req.sort_order,
            is_active: true,
        }),
    )
        .into_response()
}

async fn update_tag_definition(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateTagDefinitionRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row(
            "SELECT 1 FROM tag_definitions WHERE id = ?1",
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

    if req.value_type.is_some() {
        let referenced = match conn
            .query_row(
                "SELECT 1 FROM event_type_mutations WHERE tag_definition_id = ?1 LIMIT 1",
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
                    reason_code: "TAG_DEFINITION_TYPE_IN_USE",
                    message: "cannot mutate type while referenced".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = conn.execute(
            "UPDATE tag_definitions SET value_type = ?1 WHERE id = ?2",
            rusqlite::params![req.value_type, id],
        ) {
            return internal_error(err.to_string());
        }
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn delete_tag_definition(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row(
            "SELECT 1 FROM tag_definitions WHERE id = ?1",
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
            "SELECT 1 FROM event_type_mutations WHERE tag_definition_id = ?1 LIMIT 1",
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
                reason_code: "TAG_DEFINITION_IN_USE",
                message: "cannot delete tag definition while referenced".to_string(),
            }),
        )
            .into_response();
    }

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM category_tag_hints WHERE tag_definition_id = ?1 LIMIT 1",
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
                reason_code: "TAG_DEFINITION_IN_USE",
                message: "cannot delete tag definition while referenced".to_string(),
            }),
        )
            .into_response();
    }

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM tag_enum_options WHERE tag_definition_id = ?1 LIMIT 1",
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
                reason_code: "TAG_DEFINITION_IN_USE",
                message: "cannot delete tag definition while referenced".to_string(),
            }),
        )
            .into_response();
    }

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM asset_current_tag_values WHERE tag_definition_id = ?1 LIMIT 1",
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
                reason_code: "TAG_DEFINITION_IN_USE",
                message: "cannot delete tag definition while referenced".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(err) = conn.execute("DELETE FROM tag_definitions WHERE id = ?1", [id]) {
        return internal_error(err.to_string());
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn delete_tag_enum_option(
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

    let referenced = match conn
        .query_row(
            "SELECT 1 FROM asset_current_tag_values WHERE enum_option_id = ?1 LIMIT 1",
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
                reason_code: "TAG_ENUM_OPTION_IN_USE",
                message: "cannot delete enum option while referenced".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(err) = conn.execute("DELETE FROM tag_enum_options WHERE id = ?1", [id]) {
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
