use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{validate_tag_value, FieldType, ValidationError};

pub type DomainState = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventType {
    pub event_type_id: String,
    pub event_type_version: u32,
    pub mutations: Vec<MutationSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MutationSpec {
    Set {
        field_id: String,
        field_type: FieldType,
        input_key: String,
    },
    Clear {
        field_id: String,
    },
    Increment {
        field_id: String,
        field_type: FieldType,
        input_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub idempotency_key: String,
    pub event_type_id: String,
    pub event_type_version: u32,
    pub payload: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyComparison {
    DifferentKey,
    SamePayload,
    PayloadMismatch,
}

impl Event {
    pub fn compare_idempotency(&self, other: &Self) -> IdempotencyComparison {
        if self.idempotency_key != other.idempotency_key {
            return IdempotencyComparison::DifferentKey;
        }

        if self.event_type_id == other.event_type_id
            && self.event_type_version == other.event_type_version
            && self.payload == other.payload
        {
            IdempotencyComparison::SamePayload
        } else {
            IdempotencyComparison::PayloadMismatch
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventApplyError {
    EventTypeMismatch {
        expected_event_type_id: String,
        expected_event_type_version: u32,
        found_event_type_id: String,
        found_event_type_version: u32,
    },
    MissingInput {
        input_key: String,
    },
    InvalidInputValue {
        input_key: String,
        field_type: FieldType,
        reason: ValidationError,
    },
    InvalidIncrementTargetType {
        field_id: String,
        field_type: FieldType,
    },
    MixedNumericKinds {
        field_id: String,
    },
    IntegerOverflow {
        field_id: String,
    },
}

impl core::fmt::Display for EventApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EventApplyError::EventTypeMismatch {
                expected_event_type_id,
                expected_event_type_version,
                found_event_type_id,
                found_event_type_version,
            } => write!(
                f,
                "event type mismatch: expected {}@{}, got {}@{}",
                expected_event_type_id,
                expected_event_type_version,
                found_event_type_id,
                found_event_type_version
            ),
            EventApplyError::MissingInput { input_key } => {
                write!(f, "missing required input key '{}'", input_key)
            }
            EventApplyError::InvalidInputValue {
                input_key,
                field_type,
                reason,
            } => {
                write!(
                    f,
                    "invalid input '{}' for {:?}: {}",
                    input_key, field_type, reason
                )
            }
            EventApplyError::InvalidIncrementTargetType {
                field_id,
                field_type,
            } => {
                write!(
                    f,
                    "increment on '{}' is invalid for field type {:?}",
                    field_id, field_type
                )
            }
            EventApplyError::MixedNumericKinds { field_id } => {
                write!(
                    f,
                    "increment for '{}' mixed integer/decimal kinds",
                    field_id
                )
            }
            EventApplyError::IntegerOverflow { field_id } => {
                write!(f, "integer increment overflow for '{}'", field_id)
            }
        }
    }
}

impl std::error::Error for EventApplyError {}

pub fn apply_event(
    state: &mut DomainState,
    event_type: &EventType,
    event: &Event,
) -> Result<(), EventApplyError> {
    if event.event_type_id != event_type.event_type_id
        || event.event_type_version != event_type.event_type_version
    {
        return Err(EventApplyError::EventTypeMismatch {
            expected_event_type_id: event_type.event_type_id.clone(),
            expected_event_type_version: event_type.event_type_version,
            found_event_type_id: event.event_type_id.clone(),
            found_event_type_version: event.event_type_version,
        });
    }

    let mut working = state.clone();

    for mutation in &event_type.mutations {
        match mutation {
            MutationSpec::Set {
                field_id,
                field_type,
                input_key,
            } => {
                let input =
                    event
                        .payload
                        .get(input_key)
                        .ok_or_else(|| EventApplyError::MissingInput {
                            input_key: input_key.clone(),
                        })?;

                validate_tag_value(field_type, input).map_err(|reason| {
                    EventApplyError::InvalidInputValue {
                        input_key: input_key.clone(),
                        field_type: field_type.clone(),
                        reason,
                    }
                })?;

                working.insert(field_id.clone(), input.clone());
            }
            MutationSpec::Clear { field_id } => {
                working.remove(field_id);
            }
            MutationSpec::Increment {
                field_id,
                field_type,
                input_key,
            } => {
                let input =
                    event
                        .payload
                        .get(input_key)
                        .ok_or_else(|| EventApplyError::MissingInput {
                            input_key: input_key.clone(),
                        })?;

                apply_increment(&mut working, field_id, field_type, input_key, input)?;
            }
        }
    }

    *state = working;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericKind {
    Integer,
    Decimal,
}

fn numeric_kind(value: &Value) -> Option<NumericKind> {
    let n = value.as_number()?;
    if n.as_i64().is_some() || n.as_u64().is_some() {
        Some(NumericKind::Integer)
    } else if n.as_f64().is_some() {
        Some(NumericKind::Decimal)
    } else {
        None
    }
}

fn apply_increment(
    state: &mut DomainState,
    field_id: &str,
    field_type: &FieldType,
    input_key: &str,
    input: &Value,
) -> Result<(), EventApplyError> {
    let current = state.get(field_id);

    match field_type {
        FieldType::Integer => {
            let delta = parse_integer_increment_input(field_id, field_type, input_key, input)?;
            let base = parse_integer_current_value(field_id, current)?;
            let sum = base
                .checked_add(delta)
                .ok_or_else(|| EventApplyError::IntegerOverflow {
                    field_id: field_id.to_string(),
                })?;
            let stored =
                json_integer_from_i128(sum).ok_or_else(|| EventApplyError::IntegerOverflow {
                    field_id: field_id.to_string(),
                })?;
            state.insert(field_id.to_string(), stored);
            Ok(())
        }
        FieldType::Decimal => {
            let delta = parse_decimal_increment_input(field_id, field_type, input_key, input)?;
            let base = current.and_then(Value::as_f64).unwrap_or(0.0);

            if current.is_some() && current.and_then(numeric_kind) != Some(NumericKind::Decimal) {
                return Err(EventApplyError::MixedNumericKinds {
                    field_id: field_id.to_string(),
                });
            }

            let sum = base + delta;
            let out = serde_json::Number::from_f64(sum).ok_or_else(|| {
                EventApplyError::MixedNumericKinds {
                    field_id: field_id.to_string(),
                }
            })?;
            state.insert(field_id.to_string(), Value::Number(out));
            Ok(())
        }
        _ => Err(EventApplyError::InvalidIncrementTargetType {
            field_id: field_id.to_string(),
            field_type: field_type.clone(),
        }),
    }
}

fn parse_integer_increment_input(
    field_id: &str,
    field_type: &FieldType,
    input_key: &str,
    input: &Value,
) -> Result<i128, EventApplyError> {
    let Some(kind) = numeric_kind(input) else {
        return Err(EventApplyError::InvalidInputValue {
            input_key: input_key.to_string(),
            field_type: field_type.clone(),
            reason: ValidationError::ExpectedNumber,
        });
    };

    if kind != NumericKind::Integer {
        return Err(EventApplyError::MixedNumericKinds {
            field_id: field_id.to_string(),
        });
    }

    json_integer_to_i128(input).ok_or_else(|| EventApplyError::InvalidInputValue {
        input_key: input_key.to_string(),
        field_type: field_type.clone(),
        reason: ValidationError::ExpectedInteger,
    })
}

fn parse_integer_current_value(
    field_id: &str,
    current: Option<&Value>,
) -> Result<i128, EventApplyError> {
    let Some(value) = current else {
        return Ok(0);
    };

    if numeric_kind(value) != Some(NumericKind::Integer) {
        return Err(EventApplyError::MixedNumericKinds {
            field_id: field_id.to_string(),
        });
    }

    json_integer_to_i128(value).ok_or_else(|| EventApplyError::MixedNumericKinds {
        field_id: field_id.to_string(),
    })
}

fn parse_decimal_increment_input(
    field_id: &str,
    field_type: &FieldType,
    input_key: &str,
    input: &Value,
) -> Result<f64, EventApplyError> {
    let Some(kind) = numeric_kind(input) else {
        return Err(EventApplyError::InvalidInputValue {
            input_key: input_key.to_string(),
            field_type: field_type.clone(),
            reason: ValidationError::ExpectedNumber,
        });
    };

    if kind != NumericKind::Decimal {
        return Err(EventApplyError::MixedNumericKinds {
            field_id: field_id.to_string(),
        });
    }

    input
        .as_f64()
        .ok_or_else(|| EventApplyError::InvalidInputValue {
            input_key: input_key.to_string(),
            field_type: field_type.clone(),
            reason: ValidationError::ExpectedNumber,
        })
}

fn json_integer_to_i128(value: &Value) -> Option<i128> {
    if let Some(v) = value.as_i64() {
        Some(v as i128)
    } else {
        value.as_u64().map(|v| v as i128)
    }
}

fn json_integer_from_i128(value: i128) -> Option<Value> {
    if value >= 0 {
        let v = u64::try_from(value).ok()?;
        Some(Value::from(v))
    } else {
        let v = i64::try_from(value).ok()?;
        Some(Value::from(v))
    }
}
