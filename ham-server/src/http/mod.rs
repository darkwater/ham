use axum::{
    Router,
    routing::{delete, get, patch, post},
};

mod assets;
mod categories;
mod fields;

pub async fn run(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/assets", get(crate::http::assets::list_assets))
        .route("/assets", post(crate::http::assets::create_asset))
        .route("/assets/{id}", get(crate::http::assets::get_asset))
        .route("/assets/{id}", patch(crate::http::assets::update_asset))
        .route("/categories", get(crate::http::categories::list_categories))
        .route("/categories", post(crate::http::categories::create_category))
        .route("/categories/{id}", delete(crate::http::categories::delete_category))
        .route("/fields", get(crate::http::fields::list_fields))
        .route("/fields", post(crate::http::fields::create_field))
        .with_state(pool)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6172").await?;
    axum::serve(listener, app).await?;

    unreachable!("axum::serve claims to never return")
}
