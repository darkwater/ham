use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Integer,
    Decimal,
    Boolean,
    Date,
    Datetime,
    Money,
    Url,
    MacAddress,
    Ipv4,
    Enum,
    ExternalEntity(i64),
}
