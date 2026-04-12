use axum::{Json, extract::State, http::StatusCode};
use ham_shared::{Category, CategoryId, CreateCategoryParams};

pub async fn list_categories(
    pool: State<sqlx::SqlitePool>,
) -> Result<Json<Vec<Category>>, StatusCode> {
    let categories = sqlx::query!("SELECT id, display_name, parent_category_id FROM categories")
        .map(|row| Category {
            id: CategoryId(row.id),
            display_name: row.display_name,
            parent_id: row.parent_category_id.map(CategoryId),
        })
        .fetch_all(&*pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(categories))
}

pub async fn create_category(
    pool: State<sqlx::SqlitePool>,
    params: Json<CreateCategoryParams>,
) -> Result<Json<Category>, StatusCode> {
    if let Some(CategoryId(parent_id)) = params.parent_id {
        let parent_exists = sqlx::query!("SELECT id FROM categories WHERE id = ?", parent_id)
            .fetch_optional(&*pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some();

        if !parent_exists {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let category_id = sqlx::query!(
        "INSERT INTO categories (display_name, parent_category_id) VALUES (?, ?)",
        params.display_name,
        params.parent_id.map(|id| id.0),
    )
    .execute(&*pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .last_insert_rowid();

    Ok(Json(Category {
        id: CategoryId(category_id),
        display_name: params.display_name.clone(),
        parent_id: params.parent_id,
    }))
}
