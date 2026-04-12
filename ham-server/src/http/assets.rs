use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_extra::extract::Query;
use ham_shared::{
    Asset, AssetField, AssetId, CategoryId, CreateAssetParams, FieldId, ListAssetParams,
};
use sqlx::{AssertSqlSafe, Row as _};

pub async fn list_assets(
    Query(params): Query<ListAssetParams>,
    State(pool): State<sqlx::SqlitePool>,
) -> Result<Json<Vec<Asset>>, StatusCode> {
    let ListAssetParams { ref field_ids } = params;

    let field_ids = field_ids.to_vec();

    let query = format!(
        "SELECT a.id, a.category_id, a.display_name, afv.field_id, afv.value
         FROM assets a
         LEFT JOIN asset_field_values afv
           ON afv.asset_id = a.id
          AND afv.field_id IN ({})",
        vec!["?"; field_ids.len()].join(","),
    );

    // the only dynamic part can only look like "?,?,?"
    let query = AssertSqlSafe(query);

    let mut query = sqlx::query(query);

    for FieldId(id) in field_ids {
        query = query.bind(id);
    }

    let rows = query.fetch_all(&pool).await.map_err(|e| {
        tracing::error!("Failed to query assets: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut assets = BTreeMap::new();

    for row in rows {
        let asset_id: i64 = row.get("id");
        let category_id: i64 = row.get("category_id");
        let display_name: String = row.get("display_name");
        let field_id: Option<_> = row.get("field_id");
        let value: Option<_> = row.get("value");

        let asset = assets.entry(asset_id).or_insert_with(|| Asset {
            id: AssetId(asset_id),
            category_id: CategoryId(category_id),
            display_name,
            fields: Vec::new(),
        });

        if let (Some(field_id), Some(value)) = (field_id, value) {
            asset
                .fields
                .push(AssetField { field_id: FieldId(field_id), value });
        }
    }

    Ok(Json(assets.into_values().collect()))
}

pub async fn create_asset(
    State(pool): State<sqlx::SqlitePool>,
    Json(params): Json<CreateAssetParams>,
) -> Result<Json<Asset>, StatusCode> {
    let CreateAssetParams { category_id, display_name } = params;

    let category = sqlx::query!("SELECT id FROM categories WHERE id = ?", params.category_id.0)
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if category.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let asset_id = sqlx::query!(
        "INSERT INTO assets (category_id, display_name) VALUES (?, ?)",
        category_id.0,
        display_name,
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .last_insert_rowid();

    Ok(Json(Asset {
        id: AssetId(asset_id),
        category_id,
        display_name,
        fields: vec![],
    }))
}

pub async fn get_asset(
    State(pool): State<sqlx::SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Asset>, StatusCode> {
    let asset = sqlx::query!("SELECT id, category_id, display_name FROM assets WHERE id = ?", id)
        .map(|row| Asset {
            id: AssetId(row.id),
            category_id: CategoryId(row.category_id),
            display_name: row.display_name,
            fields: vec![],
        })
        .fetch_optional(&pool)
        .await
        .unwrap();

    let Some(mut asset) = asset else {
        return Err(StatusCode::NOT_FOUND);
    };

    let fields = sqlx::query!(
            "SELECT field_id, value AS \"value: serde_json::Value\" FROM asset_field_values WHERE asset_id = ?",
            asset.id.0,
        )
        .map(|row| AssetField {
            field_id: FieldId(row.field_id),
            value: row.value,
        })
        .fetch_all(&pool)
        .await
        .unwrap();

    asset.fields = fields;
    Ok(Json(asset))
}
