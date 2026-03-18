use std::{path::PathBuf, sync::Arc};

use axum::Router;

#[derive(Clone)]
pub struct AppState {
    pub db_path: Arc<PathBuf>,
}

pub fn build_app(db_path: PathBuf) -> Result<Router, crate::db::DbError> {
    let _ = crate::db::open_and_prepare(&db_path)?;
    let state = AppState {
        db_path: Arc::new(db_path),
    };
    Ok(crate::http::router(state))
}
