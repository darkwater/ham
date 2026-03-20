use axum::{Router, routing::get};

mod assets;

pub async fn run(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/assets", get(crate::http::assets::list_assets))
        .route("/assets/{id}", get(crate::http::assets::get_asset))
        .with_state(pool)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6172").await?;
    axum::serve(listener, app).await?;

    unreachable!("axum::serve claims to never return")
}
