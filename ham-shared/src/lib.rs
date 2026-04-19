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
    pub value: FieldValue,
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
    pub field_ids: Vec<FieldId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: FieldId,
    pub display_name: String,
    pub value_type: FieldType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Int,
    Float,
    Money,
    Boolean,
    DateTime(DateTimePrecision),
    Enum(EnumId),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DateTimePrecision {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldValue {
    String(String),
    Int(i64),
    Float(f64),
    Money {
        amount: String,
        currency: String,
    },
    Boolean(bool),
    DateTime {
        date: chrono::DateTime<chrono::Utc>,
        precision: DateTimePrecision,
    },
    Enum {
        enum_id: EnumId,
        value: EnumValueId,
    },
}

impl FieldValue {
    pub fn field_type(&self) -> FieldType {
        match self {
            FieldValue::String(_) => FieldType::String,
            FieldValue::Int(_) => FieldType::Int,
            FieldValue::Float(_) => FieldType::Float,
            FieldValue::Money { .. } => FieldType::Money,
            FieldValue::Boolean(_) => FieldType::Boolean,
            FieldValue::DateTime { precision, .. } => FieldType::DateTime(*precision),
            FieldValue::Enum { enum_id, .. } => FieldType::Enum(*enum_id),
        }
    }

    // pub fn display(&self, enum_values: &[EnumValue]) -> String {
    //     match self {
    //         FieldValue::String(s) => s.clone(),
    //         FieldValue::Int(i) => i.to_string(),
    //         FieldValue::Float(fl) => fl.to_string(),
    //         FieldValue::Money { amount, currency } => format!("{} {}", currency, amount),
    //         FieldValue::Boolean(b) => b.to_string(),
    //         FieldValue::DateTime { date, precision } => {
    //             let fmt = match precision {
    //                 DateTimePrecision::Year => "%Y",
    //                 DateTimePrecision::Month => "%Y-%m",
    //                 DateTimePrecision::Day => "%Y-%m-%d",
    //                 DateTimePrecision::Hour => "%Y-%m-%d %H",
    //                 DateTimePrecision::Minute => "%Y-%m-%d %H:%M",
    //                 DateTimePrecision::Second => "%Y-%m-%d %H:%M:%S",
    //             };
    //             date.format(fmt).to_string()
    //         }
    //         FieldValue::Enum { enum_id, value } => {
    //             enum_values
    //                 .iter()
    //                 .find(|ev| ev.enum_id == *enum_id && ev.id == *value)
    //                 .map(|ev| ev.display_name.clone())
    //                 .unwrap_or_else(|| format!("Unknown enum value {value:?} for enum {enum_id:?}"))
    //         }
    //     }
    // }
}

impl Display for FieldValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldValue::String(s) => s.fmt(f),
            FieldValue::Int(i) => i.fmt(f),
            FieldValue::Float(fl) => fl.fmt(f),
            FieldValue::Money { amount, currency } => write!(f, "{} {}", currency, amount),
            FieldValue::Boolean(b) => b.fmt(f),
            FieldValue::DateTime { date, precision } => {
                let fmt = match precision {
                    DateTimePrecision::Year => "%Y",
                    DateTimePrecision::Month => "%Y-%m",
                    DateTimePrecision::Day => "%Y-%m-%d",
                    DateTimePrecision::Hour => "%Y-%m-%d %H",
                    DateTimePrecision::Minute => "%Y-%m-%d %H:%M",
                    DateTimePrecision::Second => "%Y-%m-%d %H:%M:%S",
                };
                date.format(fmt).fmt(f)
            }
            FieldValue::Enum { value, .. } => value.fmt(f),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategoryParams {
    pub display_name: String,
    pub parent_id: Option<CategoryId>,
    pub field_ids: Vec<FieldId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFieldParams {
    pub display_name: String,
    pub value_type: FieldType,
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

            #[cfg(feature = "sqlx")]
            impl sqlx::Type<sqlx::Sqlite> for $ty {
                fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                    <i64 as sqlx::Type<sqlx::Sqlite>>::type_info()
                }
                fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
                    <i64 as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
                }
            }

            #[cfg(feature = "sqlx")]
            impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $ty {
                fn decode(
                    value: sqlx::sqlite::SqliteValueRef<'r>
                ) -> Result<Self, sqlx::error::BoxDynError> {
                    <i64 as sqlx::Decode<'r, sqlx::Sqlite>>::decode(value).map(Self)
                }
            }

            #[cfg(feature = "sqlx")]
            impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $ty {
                fn encode_by_ref(
                    &self,
                    buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer,
                ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>>
                {
                    <i64 as sqlx::Encode<'q, sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
                }
            }
        )*
    };
}

newtypes!(AssetId, CategoryId, FieldId, EnumId, EnumValueId);
