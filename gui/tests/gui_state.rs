use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gui::state::{
    apply_event_url, event_type_url, timeline_url, ApiClient, ApiError, AppliedEvent, Asset,
    AssetEvent, Category, EventFormFieldKind, EventInputSpec, EventInputSpecKind, EventTypeSchema,
    ExternalEntityOption, GuiController, GuiState, TimelinePage,
};
use serde_json::json;

#[test]
fn loads_catalog_opens_asset_and_fetches_full_timeline() {
    let api = FakeApi::default();
    api.categories
        .borrow_mut()
        .push(Category::new(1, "Network", None));
    api.assets.borrow_mut().push(Asset::new(
        11,
        1,
        "AST-001",
        Some("Router".to_string()),
        None,
    ));
    api.timeline_pages.borrow_mut().insert(
        "AST-001".to_string(),
        vec![
            TimelinePage {
                items: vec![AssetEvent::new(
                    1,
                    "asset.set-owner",
                    1,
                    json!({"owner":"alice"}),
                    "2026-01-01T00:00:00Z",
                )],
                next_cursor: Some("cursor-1".to_string()),
            },
            TimelinePage {
                items: vec![AssetEvent::new(
                    2,
                    "asset.set-owner",
                    1,
                    json!({"owner":"bob"}),
                    "2026-01-02T00:00:00Z",
                )],
                next_cursor: None,
            },
        ],
    );

    let mut controller = GuiController::new(api.clone(), GuiState::default());
    controller.load_catalog().unwrap();
    controller.open_asset_detail("AST-001").unwrap();

    assert_eq!(controller.state().categories.len(), 1);
    assert_eq!(controller.state().assets.len(), 1);
    assert_eq!(
        controller.state().selected_asset_tag.as_deref(),
        Some("AST-001")
    );
    assert_eq!(controller.state().timeline.len(), 2);

    let cursors = api.requested_timeline_cursors.borrow();
    assert_eq!(cursors.as_slice(), &[None, Some("cursor-1".to_string())]);
}

#[test]
fn event_form_supports_enum_and_external_entity_selectors() {
    let api = FakeApi::default();
    api.schemas.borrow_mut().insert(
        "asset.update".to_string(),
        EventTypeSchema {
            event_type_id: "asset.update".to_string(),
            inputs: vec![
                EventInputSpec::enum_select(
                    "status",
                    vec![("active", "Active"), ("retired", "Retired")],
                ),
                EventInputSpec::external_entity_select(
                    "vendor_id",
                    vec![(7, "Vendor 7"), (8, "Vendor 8")],
                ),
            ],
        },
    );

    let state = GuiState {
        selected_asset_tag: Some("AST-001".to_string()),
        ..GuiState::default()
    };
    let mut controller = GuiController::new(api, state);
    controller.start_event_from_type("asset.update").unwrap();

    let form = controller.state().event_form.as_ref().unwrap();
    assert_eq!(form.fields.len(), 2);
    match &form.fields[0].kind {
        EventFormFieldKind::Enum(options) => {
            assert_eq!(options[0].option_key, "active");
            assert_eq!(options[1].display_name, "Retired");
        }
        other => panic!("expected enum selector, got {other:?}"),
    }
    match &form.fields[1].kind {
        EventFormFieldKind::ExternalEntity(options) => {
            assert_eq!(options[0], ExternalEntityOption::new(7, "Vendor 7"));
        }
        other => panic!("expected external selector, got {other:?}"),
    }
}

#[test]
fn apply_event_uses_schema_driven_payload_and_refreshes_timeline() {
    let api = FakeApi::default();
    api.schemas.borrow_mut().insert(
        "asset.update".to_string(),
        EventTypeSchema {
            event_type_id: "asset.update".to_string(),
            inputs: vec![
                EventInputSpec::enum_select("status", vec![("active", "Active")]),
                EventInputSpec::external_entity_select("vendor_id", vec![(21, "Vendor 21")]),
                EventInputSpec::text("owner"),
            ],
        },
    );
    api.timeline_pages.borrow_mut().insert(
        "AST-001".to_string(),
        vec![TimelinePage {
            items: vec![AssetEvent::new(
                5,
                "asset.update",
                1,
                json!({"status":"active","vendor_id":21,"owner":"ops"}),
                "2026-01-03T00:00:00Z",
            )],
            next_cursor: None,
        }],
    );

    let state = GuiState {
        selected_asset_tag: Some("AST-001".to_string()),
        ..GuiState::default()
    };
    let mut controller = GuiController::new(api.clone(), state);
    controller.start_event_from_type("asset.update").unwrap();
    controller.set_form_value("status", "active").unwrap();
    controller.set_form_value("vendor_id", "21").unwrap();
    controller.set_form_value("owner", "ops").unwrap();

    controller.apply_event("idem-1").unwrap();

    let payload = api.applied_payload.borrow().clone().unwrap();
    assert_eq!(payload.get("status"), Some(&json!("active")));
    assert_eq!(payload.get("vendor_id"), Some(&json!(21)));
    assert_eq!(payload.get("owner"), Some(&json!("ops")));
    assert_eq!(controller.state().timeline.len(), 1);
    assert_eq!(controller.state().timeline[0].event_id, 5);
}

#[test]
fn open_asset_detail_failure_does_not_partially_mutate_state() {
    let api = FakeApi::default();
    api.fail_timeline_for
        .borrow_mut()
        .insert("AST-BAD".to_string());

    let state = GuiState {
        selected_asset_tag: Some("AST-OLD".to_string()),
        timeline: vec![AssetEvent::new(
            99,
            "asset.old",
            1,
            json!({"ok":true}),
            "2026-01-01T00:00:00Z",
        )],
        ..GuiState::default()
    };

    let mut controller = GuiController::new(api, state);
    let before = controller.state().clone();
    let err = controller.open_asset_detail("AST-BAD").unwrap_err();
    assert!(matches!(err, ApiError::Invalid(_)));
    assert_eq!(
        controller.state().selected_asset_tag,
        before.selected_asset_tag
    );
    assert_eq!(controller.state().timeline, before.timeline);
}

#[test]
fn apply_event_rejects_invalid_integer_payload_value() {
    let api = FakeApi::default();
    api.schemas.borrow_mut().insert(
        "asset.count".to_string(),
        EventTypeSchema {
            event_type_id: "asset.count".to_string(),
            inputs: vec![EventInputSpec {
                input_key: "count".to_string(),
                label: "count".to_string(),
                kind: EventInputSpecKind::Integer,
            }],
        },
    );

    let state = GuiState {
        selected_asset_tag: Some("AST-001".to_string()),
        ..GuiState::default()
    };
    let mut controller = GuiController::new(api, state);
    controller.start_event_from_type("asset.count").unwrap();
    controller.set_form_value("count", "abc").unwrap();

    let err = controller.apply_event("idem-int").unwrap_err();
    assert!(matches!(err, ApiError::Invalid(message) if message.contains("count must be integer")));
}

#[test]
fn apply_event_rejects_invalid_decimal_payload_value() {
    let api = FakeApi::default();
    api.schemas.borrow_mut().insert(
        "asset.cost".to_string(),
        EventTypeSchema {
            event_type_id: "asset.cost".to_string(),
            inputs: vec![EventInputSpec {
                input_key: "amount".to_string(),
                label: "amount".to_string(),
                kind: EventInputSpecKind::Decimal,
            }],
        },
    );

    let state = GuiState {
        selected_asset_tag: Some("AST-001".to_string()),
        ..GuiState::default()
    };
    let mut controller = GuiController::new(api, state);
    controller.start_event_from_type("asset.cost").unwrap();
    controller.set_form_value("amount", "nan-ish").unwrap();

    let err = controller.apply_event("idem-dec").unwrap_err();
    assert!(
        matches!(err, ApiError::Invalid(message) if message.contains("amount must be decimal"))
    );
}

#[test]
fn apply_event_rejects_invalid_boolean_payload_value() {
    let api = FakeApi::default();
    api.schemas.borrow_mut().insert(
        "asset.flag".to_string(),
        EventTypeSchema {
            event_type_id: "asset.flag".to_string(),
            inputs: vec![EventInputSpec {
                input_key: "enabled".to_string(),
                label: "enabled".to_string(),
                kind: EventInputSpecKind::Boolean,
            }],
        },
    );

    let state = GuiState {
        selected_asset_tag: Some("AST-001".to_string()),
        ..GuiState::default()
    };
    let mut controller = GuiController::new(api, state);
    controller.start_event_from_type("asset.flag").unwrap();
    controller.set_form_value("enabled", "maybe").unwrap();

    let err = controller.apply_event("idem-bool").unwrap_err();
    assert!(
        matches!(err, ApiError::Invalid(message) if message.contains("enabled must be boolean"))
    );
}

#[test]
fn url_builders_percent_encode_special_characters() {
    let base = "http://127.0.0.1:3000";
    let timeline =
        timeline_url(base, "AST /#?%", Some("n@xt + &=?")).expect("timeline url should build");
    let event_type = event_type_url(base, "asset/type ?v=1").expect("event type url should build");
    let apply = apply_event_url(base, "AST /#?%").expect("apply url should build");

    assert_eq!(
        timeline,
        "http://127.0.0.1:3000/assets/AST%20%2F%23%3F%25/events?limit=100&cursor=n%40xt+%2B+%26%3D%3F"
    );
    assert_eq!(
        event_type,
        "http://127.0.0.1:3000/event-types/asset%2Ftype%20%3Fv=1"
    );
    assert_eq!(
        apply,
        "http://127.0.0.1:3000/assets/AST%20%2F%23%3F%25/events"
    );
}

#[derive(Clone, Default)]
struct FakeApi {
    categories: Rc<RefCell<Vec<Category>>>,
    assets: Rc<RefCell<Vec<Asset>>>,
    timeline_pages: Rc<RefCell<BTreeMap<String, Vec<TimelinePage>>>>,
    schemas: Rc<RefCell<BTreeMap<String, EventTypeSchema>>>,
    requested_timeline_cursors: Rc<RefCell<Vec<Option<String>>>>,
    applied_payload: Rc<RefCell<Option<serde_json::Map<String, serde_json::Value>>>>,
    fail_timeline_for: Rc<RefCell<std::collections::BTreeSet<String>>>,
}

impl ApiClient for FakeApi {
    fn fetch_categories(&self) -> Result<Vec<Category>, ApiError> {
        Ok(self.categories.borrow().clone())
    }

    fn fetch_assets(&self) -> Result<Vec<Asset>, ApiError> {
        Ok(self.assets.borrow().clone())
    }

    fn fetch_timeline_page(
        &self,
        asset_tag: &str,
        cursor: Option<&str>,
    ) -> Result<TimelinePage, ApiError> {
        if self.fail_timeline_for.borrow().contains(asset_tag) {
            return Err(ApiError::invalid("timeline fetch failure"));
        }
        self.requested_timeline_cursors
            .borrow_mut()
            .push(cursor.map(ToOwned::to_owned));
        let mut pages = self.timeline_pages.borrow_mut();
        let sequence = pages
            .get_mut(asset_tag)
            .ok_or_else(|| ApiError::invalid("timeline not seeded"))?;
        if sequence.is_empty() {
            return Ok(TimelinePage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        Ok(sequence.remove(0))
    }

    fn fetch_event_type_schema(&self, event_type_id: &str) -> Result<EventTypeSchema, ApiError> {
        self.schemas
            .borrow()
            .get(event_type_id)
            .cloned()
            .ok_or_else(|| ApiError::invalid("missing schema"))
    }

    fn apply_event(
        &self,
        _asset_tag: &str,
        _event_type_id: &str,
        payload: serde_json::Map<String, serde_json::Value>,
        _idempotency_key: &str,
    ) -> Result<AppliedEvent, ApiError> {
        *self.applied_payload.borrow_mut() = Some(payload.clone());
        Ok(AppliedEvent {
            event_id: 5,
            event_type_id: "asset.update".to_string(),
            event_type_version: 1,
            payload: serde_json::Value::Object(payload),
            timestamp: "2026-01-03T00:00:00Z".to_string(),
        })
    }
}
