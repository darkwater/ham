mod comma_separated;

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

pub use crate::comma_separated::CommaSeparated;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub category_id: CategoryId,
    pub display_name: String,
    pub fields: Vec<AssetField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetField {
    pub field_id: FieldId,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAssetParams {
    pub field_ids: CommaSeparated<FieldId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssetParams {
    pub category_id: CategoryId,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub display_name: String,
    pub parent_id: Option<CategoryId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategoryParams {
    pub display_name: String,
    pub parent_id: Option<CategoryId>,
}

macro_rules! newtypes {
    ($($ty:ident),*) => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $ty(pub i64);

            impl FromStr for $ty {
                type Err = <i64 as FromStr>::Err;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    s.parse::<i64>().map(Self)
                }
            }

            impl Display for $ty {
                fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                    self.0.fmt(f)
                }
            }
        )*
    };
}

newtypes!(AssetId, CategoryId, FieldId);
