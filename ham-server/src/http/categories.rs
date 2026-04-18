use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use ham_shared::{Category, CategoryId, CreateCategoryParams};

pub async fn list_categories(
    State(pool): State<sqlx::SqlitePool>,
) -> Result<Json<Vec<Category>>, StatusCode> {
    let mut categories =
        sqlx::query!("SELECT id, display_name, parent_category_id FROM categories")
            .map(|row| Category {
                id: CategoryId(row.id),
                display_name: row.display_name,
                parent_id: row.parent_category_id.map(CategoryId),
                field_ids: Vec::new(), // we'll fill this in later
            })
            .fetch_all(&pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let category_fields = sqlx::query!("SELECT category_id, field_id FROM category_field_hints")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for category in &mut categories {
        category.field_ids = category_fields
            .iter()
            .filter(|cf| cf.category_id == category.id.0)
            .map(|cf| ham_shared::FieldId(cf.field_id))
            .collect();
    }

    Ok(Json(categories))
}

pub async fn create_category(
    State(pool): State<sqlx::SqlitePool>,
    Json(params): Json<CreateCategoryParams>,
) -> Result<Json<Category>, StatusCode> {
    let CreateCategoryParams { display_name, parent_id, field_ids } = params;

    if let Some(CategoryId(parent_id)) = parent_id {
        let parent_exists = sqlx::query!("SELECT id FROM categories WHERE id = ?", parent_id)
            .fetch_optional(&pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some();

        if !parent_exists {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let category_id = sqlx::query!(
        "INSERT INTO categories (display_name, parent_category_id) VALUES (?, ?)",
        display_name,
        parent_id.map(|id| id.0),
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .last_insert_rowid();

    for field_id in &field_ids {
        sqlx::query!(
            "INSERT INTO category_field_hints (category_id, field_id) VALUES (?, ?)",
            category_id,
            field_id.0,
        )
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(Category {
        id: CategoryId(category_id),
        display_name,
        parent_id,
        field_ids,
    }))
}

pub async fn delete_category(
    State(pool): State<sqlx::SqlitePool>,
    Path(category_id): Path<CategoryId>,
) -> Result<StatusCode, StatusCode> {
    // check if there are any assets in this category
    let has_assets = sqlx::query!("SELECT 1 as a FROM assets WHERE category_id = ?", category_id.0)
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_some();

    if has_assets {
        return Err(StatusCode::BAD_REQUEST);
    }

    // check if there are any subcategories
    let has_subcategories =
        sqlx::query!("SELECT 1 as a FROM categories WHERE parent_category_id = ?", category_id.0)
            .fetch_optional(&pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some();

    if has_subcategories {
        return Err(StatusCode::BAD_REQUEST);
    }

    sqlx::query!("DELETE FROM categories WHERE id = ?", category_id.0)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
