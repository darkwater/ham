use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/categories", get(list_categories).post(create_category))
        .route("/categories/:id", delete(delete_category))
}

#[derive(Deserialize)]
struct CreateCategoryRequest {
    slug: String,
    name: String,
    #[serde(default)]
    parent_category_id: Option<i64>,
}

#[derive(Serialize)]
struct CategoryResponse {
    id: i64,
    slug: String,
    name: String,
    parent_category_id: Option<i64>,
}

#[derive(Serialize)]
struct CategoryListResponse {
    items: Vec<CategoryResponse>,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn create_category(
    State(state): State<AppState>,
    Json(req): Json<CreateCategoryRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let inserted = conn.execute(
        "INSERT INTO categories (slug, name, parent_category_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![req.slug, req.name, req.parent_category_id],
    );

    if let Err(err) = inserted {
        if let rusqlite::Error::SqliteFailure(code, _) = &err {
            if code.code == rusqlite::ErrorCode::ConstraintViolation {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorBody {
                        reason_code: "CATEGORY_CREATE_CONFLICT",
                        message: "category create constraint violation".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        return internal_error(err.to_string());
    }

    let id = conn.last_insert_rowid();
    (
        StatusCode::CREATED,
        Json(CategoryResponse {
            id,
            slug: req.slug,
            name: req.name,
            parent_category_id: req.parent_category_id,
        }),
    )
        .into_response()
}

async fn list_categories(State(state): State<AppState>) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let mut stmt = match conn.prepare(
        "SELECT id, slug, name, parent_category_id FROM categories ORDER BY slug ASC, id ASC",
    ) {
        Ok(stmt) => stmt,
        Err(err) => return internal_error(err.to_string()),
    };

    let rows = match stmt.query_map([], |row| {
        Ok(CategoryResponse {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            parent_category_id: row.get(3)?,
        })
    }) {
        Ok(rows) => rows,
        Err(err) => return internal_error(err.to_string()),
    };

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(category) => items.push(category),
            Err(err) => return internal_error(err.to_string()),
        }
    }

    (StatusCode::OK, Json(CategoryListResponse { items })).into_response()
}

async fn delete_category(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let exists = match conn
        .query_row(
            "SELECT 1 FROM categories WHERE id = ?1",
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

    let has_children = match conn
        .query_row(
            "SELECT 1 FROM categories WHERE parent_category_id = ?1 LIMIT 1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if has_children {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "CATEGORY_HAS_CHILDREN",
                message: "category has child categories".to_string(),
            }),
        )
            .into_response();
    }

    let has_assets = match conn
        .query_row(
            "SELECT 1 FROM assets WHERE category_id = ?1 LIMIT 1",
            [id],
            |_row| Ok(()),
        )
        .optional()
    {
        Ok(v) => v.is_some(),
        Err(err) => return internal_error(err.to_string()),
    };
    if has_assets {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody {
                reason_code: "CATEGORY_HAS_ASSETS",
                message: "category has assigned assets".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(err) = conn.execute("DELETE FROM categories WHERE id = ?1", [id]) {
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
