use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", post(create_asset).get(list_assets))
        .route(
            "/assets/:id",
            get(get_asset).patch(update_asset).delete(delete_asset),
        )
}

#[derive(Deserialize)]
struct CreateAssetRequest {
    category_id: i64,
    #[serde(default)]
    asset_tag: Option<String>,
}

#[derive(Serialize)]
struct AssetResponse {
    id: i64,
    category_id: i64,
    asset_tag: String,
    display_name: Option<String>,
    deleted_at: Option<String>,
}

#[derive(Serialize)]
struct AssetListResponse {
    items: Vec<AssetResponse>,
}

#[derive(Deserialize)]
struct AssetListQuery {
    #[serde(default)]
    include_deleted: bool,
}

#[derive(Deserialize)]
struct UpdateAssetRequest {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    clear_display_name: bool,
    #[serde(default)]
    tag_values: Option<Value>,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn create_asset(
    State(state): State<AppState>,
    Json(req): Json<CreateAssetRequest>,
) -> impl IntoResponse {
    let mut conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    match db::repo_assets::create_asset(&mut conn, req.category_id, req.asset_tag.as_deref()) {
        Ok(created) => (
            StatusCode::CREATED,
            Json(AssetResponse {
                id: created.id,
                category_id: created.category_id,
                asset_tag: created.asset_tag,
                display_name: None,
                deleted_at: None,
            }),
        )
            .into_response(),
        Err(db::repo_assets::AssetCreateError::DuplicateAssetTag(tag)) => (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "DUPLICATE_ASSET_TAG",
                message: format!("asset tag `{tag}` already exists"),
            }),
        )
            .into_response(),
        Err(err) => internal_error(err.to_string()),
    }
}

async fn list_assets(
    State(state): State<AppState>,
    Query(query): Query<AssetListQuery>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let sql = if query.include_deleted {
        "SELECT id, category_id, asset_tag, display_name, deleted_at FROM assets ORDER BY id"
    } else {
        "SELECT id, category_id, asset_tag, display_name, deleted_at FROM assets WHERE deleted_at IS NULL ORDER BY id"
    };

    let mut stmt = match conn.prepare(sql) {
        Ok(stmt) => stmt,
        Err(err) => return internal_error(err.to_string()),
    };
    let rows = match stmt.query_map([], |row| {
        Ok(AssetResponse {
            id: row.get(0)?,
            category_id: row.get(1)?,
            asset_tag: row.get(2)?,
            display_name: row.get(3)?,
            deleted_at: row.get(4)?,
        })
    }) {
        Ok(rows) => rows,
        Err(err) => return internal_error(err.to_string()),
    };

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(asset) => items.push(asset),
            Err(err) => return internal_error(err.to_string()),
        }
    }

    (StatusCode::OK, Json(AssetListResponse { items })).into_response()
}

async fn get_asset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<AssetListQuery>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let sql = if query.include_deleted {
        "SELECT id, category_id, asset_tag, display_name, deleted_at FROM assets WHERE id = ?1"
    } else {
        "SELECT id, category_id, asset_tag, display_name, deleted_at FROM assets WHERE id = ?1 AND deleted_at IS NULL"
    };

    let asset = match conn
        .query_row(sql, [id], |row| {
            Ok(AssetResponse {
                id: row.get(0)?,
                category_id: row.get(1)?,
                asset_tag: row.get(2)?,
                display_name: row.get(3)?,
                deleted_at: row.get(4)?,
            })
        })
        .optional()
    {
        Ok(asset) => asset,
        Err(err) => return internal_error(err.to_string()),
    };

    match asset {
        Some(asset) => (StatusCode::OK, Json(asset)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn update_asset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateAssetRequest>,
) -> impl IntoResponse {
    if req.tag_values.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "ASSET_TAG_VALUES_UPDATE_FORBIDDEN",
                message: "tag values must be updated via event apply".to_string(),
            }),
        )
            .into_response();
    }

    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let deleted_at: Option<String> = match conn
        .query_row("SELECT deleted_at FROM assets WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()
    {
        Ok(value) => match value {
            Some(v) => v,
            None => return StatusCode::NOT_FOUND.into_response(),
        },
        Err(err) => return internal_error(err.to_string()),
    };

    if deleted_at.is_some() {
        return (
            StatusCode::GONE,
            Json(ErrorBody {
                reason_code: "ASSET_SOFT_DELETED",
                message: "asset is soft-deleted".to_string(),
            }),
        )
            .into_response();
    }

    if req.clear_display_name {
        if let Err(err) = conn.execute("UPDATE assets SET display_name = NULL WHERE id = ?1", [id])
        {
            return internal_error(err.to_string());
        }
    } else if let Some(display_name) = req.display_name {
        if let Err(err) = conn.execute(
            "UPDATE assets SET display_name = ?1 WHERE id = ?2",
            params![display_name, id],
        ) {
            return internal_error(err.to_string());
        }
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn delete_asset(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row("SELECT 1 FROM assets WHERE id = ?1", [id], |_row| Ok(()))
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Err(err) = conn.execute(
        "UPDATE assets SET deleted_at = COALESCE(deleted_at, CURRENT_TIMESTAMP) WHERE id = ?1",
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
