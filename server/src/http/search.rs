use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, db};

pub fn routes() -> Router<AppState> {
    Router::new().route("/assets/search", post(search_assets))
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    #[serde(default)]
    filters: Vec<SearchFilter>,
    #[serde(default)]
    or_groups: Vec<SearchFilterGroup>,
    #[serde(default)]
    sort: Vec<SearchSort>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    include_total_estimate: bool,
}

#[derive(Debug, Deserialize)]
struct SearchFilterGroup {
    #[serde(default)]
    filters: Vec<SearchFilter>,
}

#[derive(Debug, Deserialize)]
struct SearchFilter {
    field: String,
    op: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    values: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    include_subtree: bool,
}

#[derive(Debug, Deserialize)]
struct SearchSort {
    field: String,
    direction: String,
}

#[derive(Debug, Serialize)]
struct SearchResponseItem {
    id: i64,
    category_id: i64,
    asset_tag: String,
    display_name: Option<String>,
    deleted_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    items: Vec<SearchResponseItem>,
    next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_estimate: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    reason_code: &'static str,
    message: String,
}

async fn search_assets(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> impl IntoResponse {
    let conn = match db::open_and_prepare(&state.db_path) {
        Ok(conn) => conn,
        Err(err) => return internal_error(err.to_string()),
    };

    let query = db::repo_assets::AssetSearchQuery {
        filters: req
            .filters
            .into_iter()
            .map(|f| db::repo_assets::AssetSearchFilter {
                field: f.field,
                op: f.op,
                value: f.value,
                values: f.values,
                include_subtree: f.include_subtree,
            })
            .collect(),
        or_groups: req
            .or_groups
            .into_iter()
            .map(|group| db::repo_assets::AssetSearchFilterGroup {
                filters: group
                    .filters
                    .into_iter()
                    .map(|f| db::repo_assets::AssetSearchFilter {
                        field: f.field,
                        op: f.op,
                        value: f.value,
                        values: f.values,
                        include_subtree: f.include_subtree,
                    })
                    .collect(),
            })
            .collect(),
        sort: req
            .sort
            .into_iter()
            .map(|s| db::repo_assets::AssetSearchSort {
                field: s.field,
                direction: s.direction,
            })
            .collect(),
        limit: req.limit,
        cursor: req.cursor,
        include_total_estimate: req.include_total_estimate,
    };

    match db::repo_assets::search_assets(&conn, &query) {
        Ok(page) => (
            StatusCode::OK,
            Json(SearchResponse {
                items: page
                    .items
                    .into_iter()
                    .map(|item| SearchResponseItem {
                        id: item.id,
                        category_id: item.category_id,
                        asset_tag: item.asset_tag,
                        display_name: item.display_name,
                        deleted_at: item.deleted_at,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
                total_estimate: page.total_estimate,
            }),
        )
            .into_response(),
        Err(db::repo_assets::AssetSearchError::InvalidRequest(message)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                reason_code: "INVALID_SEARCH_REQUEST",
                message,
            }),
        )
            .into_response(),
        Err(db::repo_assets::AssetSearchError::Sql(err)) => internal_error(err.to_string()),
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
