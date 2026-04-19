use axum::{Json, extract::State, http::StatusCode};
use ham_shared::{CreateFieldParams, Field, FieldId};

pub async fn list_fields(
    State(pool): State<sqlx::SqlitePool>,
) -> Result<Json<Vec<Field>>, StatusCode> {
    let fields = sqlx::query!("SELECT id, display_name, value_type FROM field_definitions")
        .try_map(|row| {
            let id = FieldId(row.id);

            let value_type = serde_json::from_str(&row.value_type)
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

pub async fn create_field(
    State(pool): State<sqlx::SqlitePool>,
    Json(CreateFieldParams { display_name, value_type }): Json<CreateFieldParams>,
) -> Result<Json<Field>, StatusCode> {
    let value_type_str = serde_json::to_string(&value_type)
        .inspect_err(|e| {
            tracing::error!("Failed to serialize value_type: {e}");
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let field_id = sqlx::query!(
        "INSERT INTO field_definitions (display_name, value_type) VALUES (?, ?)",
        display_name,
        value_type_str,
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .last_insert_rowid();

    Ok(Json(Field {
        id: FieldId(field_id),
        display_name,
        value_type,
    }))
}
