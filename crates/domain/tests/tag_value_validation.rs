use domain::{validate_tag_value, FieldType, StoredTagValue, ValidationError};
use serde_json::json;

#[test]
fn field_type_serializes_unit_variant_as_string() {
    let value = serde_json::to_value(FieldType::Text).unwrap();
    assert_eq!(value, json!("text"));
}

#[test]
fn field_type_serializes_external_entity_as_object() {
    let value = serde_json::to_value(FieldType::ExternalEntity(4)).unwrap();
    assert_eq!(value, json!({ "external_entity": 4 }));
}

#[test]
fn stored_tag_value_rejects_json_null() {
    let err = StoredTagValue::new(serde_json::Value::Null).unwrap_err();
    assert_eq!(err, ValidationError::NullValueNotAllowed);
}

#[test]
fn money_requires_decimal_string_not_number() {
    let good = json!("12.34");
    assert!(validate_tag_value(&FieldType::Money, &good).is_ok());

    let bad = json!(12.34);
    let err = validate_tag_value(&FieldType::Money, &bad).unwrap_err();
    assert_eq!(err, ValidationError::MoneyMustBeDecimalString);
}

#[test]
fn ipv4_requires_canonical_dotted_quad() {
    let good = json!("192.168.1.10");
    assert!(validate_tag_value(&FieldType::Ipv4, &good).is_ok());

    let leading_zero = json!("192.168.01.10");
    let err = validate_tag_value(&FieldType::Ipv4, &leading_zero).unwrap_err();
    assert_eq!(err, ValidationError::InvalidIpv4);

    let out_of_range = json!("192.168.1.256");
    let err = validate_tag_value(&FieldType::Ipv4, &out_of_range).unwrap_err();
    assert_eq!(err, ValidationError::InvalidIpv4);
}

#[test]
fn enum_value_must_be_string_option_key() {
    let good = json!("active");
    assert!(validate_tag_value(&FieldType::Enum, &good).is_ok());

    let bad = json!(10);
    let err = validate_tag_value(&FieldType::Enum, &bad).unwrap_err();
    assert_eq!(err, ValidationError::EnumMustBeStringOptionKey);
}

#[test]
fn external_entity_value_must_be_integer_id() {
    let good = json!(42);
    assert!(validate_tag_value(&FieldType::ExternalEntity(4), &good).is_ok());

    let bad = json!("42");
    let err = validate_tag_value(&FieldType::ExternalEntity(4), &bad).unwrap_err();
    assert_eq!(err, ValidationError::ExternalEntityMustBeIntegerId);
}
