use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/categories/:category_id/tag-hints", get(list_hints))
        .route(
            "/categories/:category_id/tag-hints/:tag_definition_id",
            put(upsert_hint).delete(delete_hint),
        )
}

#[derive(Deserialize)]
struct UpsertHintRequest {
    #[serde(default)]
    is_required: bool,
    #[serde(default)]
    sort_order: i64,
}

#[derive(Deserialize)]
struct HintQuery {
    #[serde(default)]
    inherited: bool,
}

#[derive(Serialize)]
struct HintRow {
    category_id: i64,
    tag_definition_id: i64,
    is_required: bool,
    sort_order: i64,
}

#[derive(Serialize)]
struct HintListResponse {
    items: Vec<HintRow>,
}

#[derive(Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn upsert_hint(
    State(state): State<AppState>,
    Path((category_id, tag_definition_id)): Path<(i64, i64)>,
    Json(req): Json<UpsertHintRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    if let Err(err) = conn.execute(
        "
        INSERT INTO category_tag_hints (category_id, tag_definition_id, is_required, sort_order)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(category_id, tag_definition_id)
        DO UPDATE SET
            is_required = excluded.is_required,
            sort_order = excluded.sort_order
        ",
        params![
            category_id,
            tag_definition_id,
            req.is_required as i64,
            req.sort_order
        ],
    ) {
        return internal_error(err.to_string());
    }

    (
        StatusCode::OK,
        Json(HintRow {
            category_id,
            tag_definition_id,
            is_required: req.is_required,
            sort_order: req.sort_order,
        }),
    )
        .into_response()
}

async fn delete_hint(
    State(state): State<AppState>,
    Path((category_id, tag_definition_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    if let Err(err) = conn.execute(
        "DELETE FROM category_tag_hints WHERE category_id = ?1 AND tag_definition_id = ?2",
        params![category_id, tag_definition_id],
    ) {
        return internal_error(err.to_string());
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn list_hints(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
    Query(query): Query<HintQuery>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let ids = if query.inherited {
        match collect_ancestor_ids(&conn, category_id) {
            Ok(ids) => ids,
            Err(err) => return internal_error(err.to_string()),
        }
    } else {
        vec![category_id]
    };

    let mut items = Vec::new();
    for id in ids {
        let mut stmt = match conn.prepare(
            "
            SELECT category_id, tag_definition_id, is_required, sort_order
            FROM category_tag_hints
            WHERE category_id = ?1
            ORDER BY sort_order, tag_definition_id
            ",
        ) {
            Ok(stmt) => stmt,
            Err(err) => return internal_error(err.to_string()),
        };

        let rows = match stmt.query_map([id], |row| {
            Ok(HintRow {
                category_id: row.get(0)?,
                tag_definition_id: row.get(1)?,
                is_required: row.get::<_, i64>(2)? != 0,
                sort_order: row.get(3)?,
            })
        }) {
            Ok(rows) => rows,
            Err(err) => return internal_error(err.to_string()),
        };
        for row in rows {
            match row {
                Ok(value) => items.push(value),
                Err(err) => return internal_error(err.to_string()),
            }
        }
    }

    (StatusCode::OK, Json(HintListResponse { items })).into_response()
}

fn collect_ancestor_ids(
    conn: &rusqlite::Connection,
    category_id: i64,
) -> Result<Vec<i64>, rusqlite::Error> {
    let mut ids = Vec::new();
    let mut current = Some(category_id);
    while let Some(id) = current {
        ids.push(id);
        current = conn
            .query_row(
                "SELECT parent_category_id FROM categories WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
    }
    ids.reverse();
    Ok(ids)
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
