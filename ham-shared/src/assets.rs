use serde::{Deserialize, Serialize};

// #[cfg(feature = "sqlx")]
// use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize)]
pub struct Asset {
    pub id: i64,
    pub display_name: String,
    pub fields: Vec<AssetField>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetField {
    pub field_id: i64,
    pub value: serde_json::Value,
}
