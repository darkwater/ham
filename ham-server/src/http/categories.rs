use axum::{Json, extract::State, http::StatusCode};
use ham_shared::{Category, CategoryId, CreateCategoryParams, Field, FieldId};

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

pub async fn list_fields(
    State(pool): State<sqlx::SqlitePool>,
) -> Result<Json<Vec<Field>>, StatusCode> {
    let fields = sqlx::query!("SELECT id, display_name, value_type FROM field_definitions")
        .try_map(|row| {
            let id = FieldId(row.id);

            let value_type = ron::from_str(&row.value_type)
                .inspect_err(|e| {
                    tracing::error!("Failed to parse value_type for {id:?}: {e}");
                })
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

            Ok(Field {
                id,
                display_name: row.display_name,
                value_type,
            })
        })
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(fields))
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
