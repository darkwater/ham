use std::collections::BTreeMap;

use domain::{
    apply_event, DomainState, Event, EventApplyError, EventType, FieldType, IdempotencyComparison,
    MutationSpec,
};
use serde_json::{json, Map, Value};

fn payload(entries: &[(&str, Value)]) -> Map<String, Value> {
    let mut map = Map::new();
    for (k, v) in entries {
        map.insert((*k).to_string(), v.clone());
    }
    map
}

#[test]
fn event_type_id_and_version_must_match_definition() {
    let def = EventType {
        event_type_id: "asset.adjust".to_string(),
        event_type_version: 2,
        mutations: vec![MutationSpec::Clear {
            field_id: "note".to_string(),
        }],
    };

    let event = Event {
        idempotency_key: "k1".to_string(),
        event_type_id: "asset.adjust".to_string(),
        event_type_version: 1,
        payload: Map::new(),
    };

    let mut state = DomainState::new();
    let err = apply_event(&mut state, &def, &event).unwrap_err();

    assert!(matches!(err, EventApplyError::EventTypeMismatch { .. }));
}

#[test]
fn applies_set_clear_and_increment_mutations() {
    let def = EventType {
        event_type_id: "asset.update".to_string(),
        event_type_version: 1,
        mutations: vec![
            MutationSpec::Set {
                field_id: "label".to_string(),
                field_type: FieldType::Text,
                input_key: "label".to_string(),
            },
            MutationSpec::Increment {
                field_id: "count".to_string(),
                field_type: FieldType::Integer,
                input_key: "delta".to_string(),
            },
            MutationSpec::Clear {
                field_id: "obsolete".to_string(),
            },
        ],
    };

    let event = Event {
        idempotency_key: "k2".to_string(),
        event_type_id: "asset.update".to_string(),
        event_type_version: 1,
        payload: payload(&[("label", json!("router")), ("delta", json!(3))]),
    };

    let mut state: DomainState = BTreeMap::from([
        ("count".to_string(), json!(4)),
        ("obsolete".to_string(), json!(true)),
    ]);

    apply_event(&mut state, &def, &event).unwrap();

    assert_eq!(state.get("label"), Some(&json!("router")));
    assert_eq!(state.get("count"), Some(&json!(7)));
    assert!(!state.contains_key("obsolete"));
}

#[test]
fn apply_is_atomic_when_any_mutation_is_invalid() {
    let def = EventType {
        event_type_id: "asset.atomic".to_string(),
        event_type_version: 1,
        mutations: vec![
            MutationSpec::Increment {
                field_id: "count".to_string(),
                field_type: FieldType::Integer,
                input_key: "delta".to_string(),
            },
            MutationSpec::Set {
                field_id: "label".to_string(),
                field_type: FieldType::Text,
                input_key: "label".to_string(),
            },
        ],
    };

    let event = Event {
        idempotency_key: "k3".to_string(),
        event_type_id: "asset.atomic".to_string(),
        event_type_version: 1,
        payload: payload(&[("delta", json!(2)), ("label", json!(99))]),
    };

    let mut state: DomainState = BTreeMap::from([("count".to_string(), json!(10))]);
    let before = state.clone();

    let err = apply_event(&mut state, &def, &event).unwrap_err();
    assert!(matches!(err, EventApplyError::InvalidInputValue { .. }));
    assert_eq!(state, before);
}

#[test]
fn mutation_inputs_must_exist_and_match_spec_type() {
    let def = EventType {
        event_type_id: "asset.set.status".to_string(),
        event_type_version: 1,
        mutations: vec![MutationSpec::Set {
            field_id: "status".to_string(),
            field_type: FieldType::Enum,
            input_key: "status".to_string(),
        }],
    };

    let mut state = DomainState::new();

    let missing = Event {
        idempotency_key: "k4".to_string(),
        event_type_id: "asset.set.status".to_string(),
        event_type_version: 1,
        payload: Map::new(),
    };
    let err = apply_event(&mut state, &def, &missing).unwrap_err();
    assert!(matches!(err, EventApplyError::MissingInput { .. }));

    let bad_type = Event {
        idempotency_key: "k5".to_string(),
        event_type_id: "asset.set.status".to_string(),
        event_type_version: 1,
        payload: payload(&[("status", json!(3))]),
    };
    let err = apply_event(&mut state, &def, &bad_type).unwrap_err();
    assert!(matches!(err, EventApplyError::InvalidInputValue { .. }));
}

#[test]
fn increment_requires_numeric_field_types_and_no_mixed_numeric_kinds() {
    let bad_increment_def = EventType {
        event_type_id: "asset.bad.increment".to_string(),
        event_type_version: 1,
        mutations: vec![MutationSpec::Increment {
            field_id: "label".to_string(),
            field_type: FieldType::Text,
            input_key: "delta".to_string(),
        }],
    };
    let event_bad_op = Event {
        idempotency_key: "k6".to_string(),
        event_type_id: "asset.bad.increment".to_string(),
        event_type_version: 1,
        payload: payload(&[("delta", json!(1))]),
    };
    let mut state = DomainState::new();
    let err = apply_event(&mut state, &bad_increment_def, &event_bad_op).unwrap_err();
    assert!(matches!(
        err,
        EventApplyError::InvalidIncrementTargetType { .. }
    ));

    let int_def = EventType {
        event_type_id: "asset.int.increment".to_string(),
        event_type_version: 1,
        mutations: vec![MutationSpec::Increment {
            field_id: "count".to_string(),
            field_type: FieldType::Integer,
            input_key: "delta".to_string(),
        }],
    };
    let mixed = Event {
        idempotency_key: "k7".to_string(),
        event_type_id: "asset.int.increment".to_string(),
        event_type_version: 1,
        payload: payload(&[("delta", json!(1.5))]),
    };
    let err = apply_event(&mut state, &int_def, &mixed).unwrap_err();
    assert!(matches!(err, EventApplyError::MixedNumericKinds { .. }));
}

#[test]
fn increment_integer_supports_large_u64_current_values() {
    let def = EventType {
        event_type_id: "asset.int.large".to_string(),
        event_type_version: 1,
        mutations: vec![MutationSpec::Increment {
            field_id: "count".to_string(),
            field_type: FieldType::Integer,
            input_key: "delta".to_string(),
        }],
    };

    let event = Event {
        idempotency_key: "k8".to_string(),
        event_type_id: "asset.int.large".to_string(),
        event_type_version: 1,
        payload: payload(&[("delta", json!(1))]),
    };

    let mut state: DomainState = BTreeMap::from([("count".to_string(), json!(u64::MAX - 1))]);
    apply_event(&mut state, &def, &event).unwrap();

    assert_eq!(state.get("count"), Some(&json!(u64::MAX)));
}

#[test]
fn increment_integer_overflow_returns_structured_error() {
    let def = EventType {
        event_type_id: "asset.int.overflow".to_string(),
        event_type_version: 1,
        mutations: vec![MutationSpec::Increment {
            field_id: "count".to_string(),
            field_type: FieldType::Integer,
            input_key: "delta".to_string(),
        }],
    };

    let event = Event {
        idempotency_key: "k9".to_string(),
        event_type_id: "asset.int.overflow".to_string(),
        event_type_version: 1,
        payload: payload(&[("delta", json!(1))]),
    };

    let mut state: DomainState = BTreeMap::from([("count".to_string(), json!(u64::MAX))]);
    let err = apply_event(&mut state, &def, &event).unwrap_err();
    assert!(matches!(err, EventApplyError::IntegerOverflow { .. }));
}

#[test]
fn increment_invalid_number_reports_mutation_input_key() {
    let def = EventType {
        event_type_id: "asset.int.bad-input".to_string(),
        event_type_version: 1,
        mutations: vec![MutationSpec::Increment {
            field_id: "count".to_string(),
            field_type: FieldType::Integer,
            input_key: "change".to_string(),
        }],
    };

    let event = Event {
        idempotency_key: "k10".to_string(),
        event_type_id: "asset.int.bad-input".to_string(),
        event_type_version: 1,
        payload: payload(&[("change", json!("oops"))]),
    };

    let mut state = DomainState::new();
    let err = apply_event(&mut state, &def, &event).unwrap_err();
    assert!(matches!(
        err,
        EventApplyError::InvalidInputValue { input_key, .. } if input_key == "change"
    ));
}

#[test]
fn idempotency_payload_comparison_distinguishes_match_vs_mismatch() {
    let base = Event {
        idempotency_key: "same-key".to_string(),
        event_type_id: "asset.update".to_string(),
        event_type_version: 1,
        payload: payload(&[("label", json!("switch")), ("delta", json!(2))]),
    };

    let same = Event {
        idempotency_key: "same-key".to_string(),
        event_type_id: "asset.update".to_string(),
        event_type_version: 1,
        payload: payload(&[("label", json!("switch")), ("delta", json!(2))]),
    };

    let different_payload = Event {
        idempotency_key: "same-key".to_string(),
        event_type_id: "asset.update".to_string(),
        event_type_version: 1,
        payload: payload(&[("label", json!("switch")), ("delta", json!(3))]),
    };

    assert_eq!(
        base.compare_idempotency(&same),
        IdempotencyComparison::SamePayload
    );
    assert_eq!(
        base.compare_idempotency(&different_payload),
        IdempotencyComparison::PayloadMismatch
    );
}
