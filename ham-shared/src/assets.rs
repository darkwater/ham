use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: i64,
    pub category_id: i64,
    pub display_name: String,
    pub fields: Vec<AssetField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetField {
    pub field_id: i64,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAssetParams {
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssetParams {
    pub category_id: i64,
    pub display_name: String,
}
