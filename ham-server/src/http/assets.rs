use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use ham_shared::assets::{Asset, AssetField};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListAssetParams {
    category: Option<i64>,
}

pub async fn list_assets(
    params: Query<ListAssetParams>,
    pool: State<sqlx::SqlitePool>,
) -> Result<axum::Json<Vec<Asset>>, StatusCode> {
    let mut assets = if let Some(category) = params.category {
        sqlx::query!(
            // category and its children
            "WITH RECURSIVE tc(i)
                AS (SELECT id FROM categories WHERE id = ?
                    UNION SELECT id FROM categories, tc
                        WHERE categories.parent_category_id = tc.i )
            SELECT id, display_name FROM assets WHERE category_id in tc;",
            category,
        )
        .map(|row| Asset {
            id: row.id,
            display_name: row.display_name,
            fields: vec![],
        })
        .fetch_all(&*pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        sqlx::query!("SELECT id, display_name FROM assets")
            .map(|row| Asset {
                id: row.id,
                display_name: row.display_name,
                fields: vec![],
            })
            .fetch_all(&*pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let fields = sqlx::query!(
        // category and its children
        r#"WITH RECURSIVE tc(i)
            AS (SELECT id FROM categories WHERE id = ?
                UNION SELECT id FROM categories, tc
                    WHERE categories.parent_category_id = tc.i )
        SELECT assets.id AS asset_id, acfv.field_id, acfv.value AS "value: serde_json::Value"
        FROM assets
        INNER JOIN asset_current_field_values acfv ON acfv.asset_id = assets.id
        WHERE category_id in tc"#,
        params.category.unwrap_or(1),
    )
    .map(|row| {
        (
            row.asset_id,
            AssetField {
                field_id: row.field_id,
                value: row.value,
            },
        )
    })
    .fetch_all(&*pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for (asset_id, field) in fields {
        if let Some(asset) = assets.iter_mut().find(|a| a.id == asset_id) {
            asset.fields.push(field);
        }
    }

    Ok(axum::Json(assets))
}

pub async fn get_asset(
    State(pool): State<sqlx::SqlitePool>,
    Path(id): Path<i64>,
) -> Result<axum::Json<Asset>, StatusCode> {
    let asset = sqlx::query!("SELECT id, display_name FROM assets WHERE id = ?", id)
        .map(|row| Asset {
            id: row.id,
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
            "SELECT field_id, value AS \"value: serde_json::Value\" FROM asset_current_field_values WHERE asset_id = ?",
            asset.id
        )
        .map(|row| AssetField {
            field_id: row.field_id,
            value: row.value,
        })
        .fetch_all(&pool)
        .await
        .unwrap();

    asset.fields = fields;
    Ok(axum::Json(asset))
}
