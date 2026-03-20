use sqlx::sqlite::SqliteConnectOptions;

mod http;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pool = sqlx::SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename("assets.db")
            .create_if_missing(true),
    )
    .await
    .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await?;

    crate::http::run(pool).await
}
