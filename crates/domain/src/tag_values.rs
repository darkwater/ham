use crate::errors::ValidationError;
use crate::types::FieldType;

#[derive(Debug, Clone, PartialEq)]
pub struct StoredTagValue(serde_json::Value);

impl StoredTagValue {
    pub fn new(value: serde_json::Value) -> Result<Self, ValidationError> {
        if value.is_null() {
            return Err(ValidationError::NullValueNotAllowed);
        }

        Ok(Self(value))
    }

    pub fn into_inner(self) -> serde_json::Value {
        self.0
    }
}

pub fn validate_tag_value(
    field_type: &FieldType,
    value: &serde_json::Value,
) -> Result<(), ValidationError> {
    if value.is_null() {
        return Err(ValidationError::NullValueNotAllowed);
    }

    match field_type {
        FieldType::Text
        | FieldType::Date
        | FieldType::Datetime
        | FieldType::Url
        | FieldType::MacAddress => {
            if value.is_string() {
                Ok(())
            } else {
                Err(ValidationError::ExpectedString)
            }
        }
        FieldType::Integer => {
            if value.as_i64().is_some() {
                Ok(())
            } else {
                Err(ValidationError::ExpectedInteger)
            }
        }
        FieldType::Decimal => {
            if value.is_number() {
                Ok(())
            } else {
                Err(ValidationError::ExpectedNumber)
            }
        }
        FieldType::Boolean => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(ValidationError::ExpectedBoolean)
            }
        }
        FieldType::Money => validate_money(value),
        FieldType::Ipv4 => validate_ipv4(value),
        FieldType::Enum => {
            if value.is_string() {
                Ok(())
            } else {
                Err(ValidationError::EnumMustBeStringOptionKey)
            }
        }
        FieldType::ExternalEntity(_) => {
            if value.as_i64().is_some() {
                Ok(())
            } else {
                Err(ValidationError::ExternalEntityMustBeIntegerId)
            }
        }
    }
}

fn validate_money(value: &serde_json::Value) -> Result<(), ValidationError> {
    let Some(raw) = value.as_str() else {
        return Err(ValidationError::MoneyMustBeDecimalString);
    };

    if is_decimal_string(raw) {
        Ok(())
    } else {
        Err(ValidationError::MoneyMustBeDecimalString)
    }
}

fn is_decimal_string(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let bytes = s.as_bytes();
    let mut idx = 0;

    if bytes[0] == b'+' || bytes[0] == b'-' {
        idx = 1;
        if idx == bytes.len() {
            return false;
        }
    }

    let mut seen_digit = false;
    let mut seen_dot = false;

    while idx < bytes.len() {
        let b = bytes[idx];
        if b == b'.' {
            if seen_dot {
                return false;
            }
            seen_dot = true;
        } else if b.is_ascii_digit() {
            seen_digit = true;
        } else {
            return false;
        }
        idx += 1;
    }

    seen_digit
}

fn validate_ipv4(value: &serde_json::Value) -> Result<(), ValidationError> {
    let Some(raw) = value.as_str() else {
        return Err(ValidationError::InvalidIpv4);
    };

    let mut count = 0usize;
    for part in raw.split('.') {
        count += 1;
        if !is_canonical_ipv4_octet(part) {
            return Err(ValidationError::InvalidIpv4);
        }
    }

    if count == 4 {
        Ok(())
    } else {
        Err(ValidationError::InvalidIpv4)
    }
}

fn is_canonical_ipv4_octet(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }

    if part.len() > 1 && part.starts_with('0') {
        return false;
    }

    if !part.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    match part.parse::<u16>() {
        Ok(v) => v <= 255,
        Err(_) => false,
    }
}
