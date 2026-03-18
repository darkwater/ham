pub mod assets;
pub mod categories;
pub mod category_tag_hints;
pub mod event_types;
pub mod events;
pub mod external_entities;
pub mod search;
pub mod tag_definitions;

use axum::{routing::get, Router};

use crate::app::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(assets::routes())
        .merge(categories::routes())
        .merge(category_tag_hints::routes())
        .merge(event_types::routes())
        .merge(events::routes())
        .merge(search::routes())
        .merge(tag_definitions::routes())
        .merge(external_entities::routes())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
