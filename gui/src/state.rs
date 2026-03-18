use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub parent_category_id: Option<i64>,
}

impl Category {
    pub fn new(id: i64, name: &str, parent_category_id: Option<i64>) -> Self {
        Self {
            id,
            name: name.to_string(),
            parent_category_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub id: i64,
    pub category_id: i64,
    pub asset_tag: String,
    pub display_name: Option<String>,
    pub deleted_at: Option<String>,
}

impl Asset {
    pub fn new(
        id: i64,
        category_id: i64,
        asset_tag: &str,
        display_name: Option<String>,
        deleted_at: Option<String>,
    ) -> Self {
        Self {
            id,
            category_id,
            asset_tag: asset_tag.to_string(),
            display_name,
            deleted_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetEvent {
    pub event_id: i64,
    pub event_type_id: String,
    pub event_type_version: i64,
    pub payload: Value,
    pub timestamp: String,
}

impl AssetEvent {
    pub fn new(
        event_id: i64,
        event_type_id: &str,
        event_type_version: i64,
        payload: Value,
        timestamp: &str,
    ) -> Self {
        Self {
            event_id,
            event_type_id: event_type_id.to_string(),
            event_type_version,
            payload,
            timestamp: timestamp.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelinePage {
    pub items: Vec<AssetEvent>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumOption {
    pub option_key: String,
    pub display_name: String,
}

impl EnumOption {
    pub fn new(option_key: &str, display_name: &str) -> Self {
        Self {
            option_key: option_key.to_string(),
            display_name: display_name.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalEntityOption {
    pub id: i64,
    pub display_name: String,
}

impl ExternalEntityOption {
    pub fn new(id: i64, display_name: &str) -> Self {
        Self {
            id,
            display_name: display_name.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventInputSpecKind {
    Text,
    Integer,
    Decimal,
    Boolean,
    Enum(Vec<EnumOption>),
    ExternalEntity(Vec<ExternalEntityOption>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventInputSpec {
    pub input_key: String,
    pub label: String,
    pub kind: EventInputSpecKind,
}

impl EventInputSpec {
    pub fn text(input_key: &str) -> Self {
        Self {
            input_key: input_key.to_string(),
            label: input_key.to_string(),
            kind: EventInputSpecKind::Text,
        }
    }

    pub fn enum_select(input_key: &str, options: Vec<(&str, &str)>) -> Self {
        Self {
            input_key: input_key.to_string(),
            label: input_key.to_string(),
            kind: EventInputSpecKind::Enum(
                options
                    .into_iter()
                    .map(|(key, label)| EnumOption::new(key, label))
                    .collect(),
            ),
        }
    }

    pub fn external_entity_select(input_key: &str, options: Vec<(i64, &str)>) -> Self {
        Self {
            input_key: input_key.to_string(),
            label: input_key.to_string(),
            kind: EventInputSpecKind::ExternalEntity(
                options
                    .into_iter()
                    .map(|(id, label)| ExternalEntityOption::new(id, label))
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventTypeSchema {
    pub event_type_id: String,
    pub inputs: Vec<EventInputSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventFormFieldKind {
    Text,
    Integer,
    Decimal,
    Boolean,
    Enum(Vec<EnumOption>),
    ExternalEntity(Vec<ExternalEntityOption>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventFormField {
    pub input_key: String,
    pub label: String,
    pub kind: EventFormFieldKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventFormState {
    pub event_type_id: String,
    pub fields: Vec<EventFormField>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuiState {
    pub categories: Vec<Category>,
    pub assets: Vec<Asset>,
    pub selected_asset_tag: Option<String>,
    pub timeline: Vec<AssetEvent>,
    pub event_form: Option<EventFormState>,
    pub last_error: Option<String>,
    pub event_type_draft: String,
    pub idempotency_key_draft: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedEvent {
    pub event_id: i64,
    pub event_type_id: String,
    pub event_type_version: i64,
    pub payload: Value,
    pub timestamp: String,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ApiError {
    #[error("http error: {0}")]
    Http(String),
    #[error("invalid response: {0}")]
    Invalid(String),
}

impl ApiError {
    pub fn invalid(message: &str) -> Self {
        Self::Invalid(message.to_string())
    }
}

pub trait ApiClient {
    fn fetch_categories(&self) -> Result<Vec<Category>, ApiError>;
    fn fetch_assets(&self) -> Result<Vec<Asset>, ApiError>;
    fn fetch_timeline_page(
        &self,
        asset_tag: &str,
        cursor: Option<&str>,
    ) -> Result<TimelinePage, ApiError>;
    fn fetch_event_type_schema(&self, event_type_id: &str) -> Result<EventTypeSchema, ApiError>;
    fn apply_event(
        &self,
        asset_tag: &str,
        event_type_id: &str,
        payload: Map<String, Value>,
        idempotency_key: &str,
    ) -> Result<AppliedEvent, ApiError>;
}

pub struct GuiController<A: ApiClient> {
    api: A,
    state: GuiState,
}

impl<A: ApiClient> GuiController<A> {
    pub fn new(api: A, state: GuiState) -> Self {
        Self { api, state }
    }

    pub fn state(&self) -> &GuiState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut GuiState {
        &mut self.state
    }

    pub fn load_catalog(&mut self) -> Result<(), ApiError> {
        let categories = self.api.fetch_categories()?;
        let assets = self.api.fetch_assets()?;

        self.state.categories = if categories.is_empty() {
            derive_categories_from_assets(&assets)
        } else {
            categories
        };
        self.state.assets = assets;
        Ok(())
    }

    pub fn open_asset_detail(&mut self, asset_tag: &str) -> Result<(), ApiError> {
        let timeline = self.fetch_full_timeline(asset_tag)?;
        self.state.selected_asset_tag = Some(asset_tag.to_string());
        self.state.timeline = timeline;
        Ok(())
    }

    pub fn start_event_from_type(&mut self, event_type_id: &str) -> Result<(), ApiError> {
        let schema = self.api.fetch_event_type_schema(event_type_id)?;
        self.state.event_form = Some(EventFormState {
            event_type_id: schema.event_type_id,
            fields: schema
                .inputs
                .into_iter()
                .map(|input| EventFormField {
                    input_key: input.input_key,
                    label: input.label,
                    kind: map_form_kind(input.kind),
                    value: String::new(),
                })
                .collect(),
        });
        Ok(())
    }

    pub fn set_form_value(&mut self, input_key: &str, value: &str) -> Result<(), ApiError> {
        let form = self
            .state
            .event_form
            .as_mut()
            .ok_or_else(|| ApiError::invalid("event form is not initialized"))?;
        let field = form
            .fields
            .iter_mut()
            .find(|f| f.input_key == input_key)
            .ok_or_else(|| ApiError::invalid("event form input key not found"))?;
        field.value = value.to_string();
        Ok(())
    }

    pub fn apply_event(&mut self, idempotency_key: &str) -> Result<(), ApiError> {
        let asset_tag = self
            .state
            .selected_asset_tag
            .clone()
            .ok_or_else(|| ApiError::invalid("no selected asset"))?;
        let form = self
            .state
            .event_form
            .as_ref()
            .ok_or_else(|| ApiError::invalid("event form is not initialized"))?;

        let payload = build_payload(&form.fields)?;
        let _ = self
            .api
            .apply_event(&asset_tag, &form.event_type_id, payload, idempotency_key)?;
        self.state.timeline = self.fetch_full_timeline(&asset_tag)?;
        Ok(())
    }

    fn fetch_full_timeline(&self, asset_tag: &str) -> Result<Vec<AssetEvent>, ApiError> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = self.api.fetch_timeline_page(asset_tag, cursor.as_deref())?;
            out.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(out)
    }
}

fn map_form_kind(kind: EventInputSpecKind) -> EventFormFieldKind {
    match kind {
        EventInputSpecKind::Text => EventFormFieldKind::Text,
        EventInputSpecKind::Integer => EventFormFieldKind::Integer,
        EventInputSpecKind::Decimal => EventFormFieldKind::Decimal,
        EventInputSpecKind::Boolean => EventFormFieldKind::Boolean,
        EventInputSpecKind::Enum(v) => EventFormFieldKind::Enum(v),
        EventInputSpecKind::ExternalEntity(v) => EventFormFieldKind::ExternalEntity(v),
    }
}

fn build_payload(fields: &[EventFormField]) -> Result<Map<String, Value>, ApiError> {
    let mut out = Map::new();
    for field in fields {
        let value = match &field.kind {
            EventFormFieldKind::Text => Value::String(field.value.clone()),
            EventFormFieldKind::Integer => Value::from(parse_i64(&field.value, &field.input_key)?),
            EventFormFieldKind::Decimal => {
                let parsed = field.value.parse::<f64>().map_err(|_| {
                    ApiError::invalid(&format!("{} must be decimal", field.input_key))
                })?;
                let num = serde_json::Number::from_f64(parsed).ok_or_else(|| {
                    ApiError::invalid(&format!("{} must be finite", field.input_key))
                })?;
                Value::Number(num)
            }
            EventFormFieldKind::Boolean => Value::Bool(parse_bool(&field.value, &field.input_key)?),
            EventFormFieldKind::Enum(_) => Value::String(field.value.clone()),
            EventFormFieldKind::ExternalEntity(_) => {
                Value::from(parse_i64(&field.value, &field.input_key)?)
            }
        };
        out.insert(field.input_key.clone(), value);
    }
    Ok(out)
}

fn parse_i64(raw: &str, key: &str) -> Result<i64, ApiError> {
    raw.parse::<i64>()
        .map_err(|_| ApiError::invalid(&format!("{key} must be integer")))
}

fn parse_bool(raw: &str, key: &str) -> Result<bool, ApiError> {
    match raw.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(ApiError::invalid(&format!("{key} must be boolean"))),
    }
}

fn derive_categories_from_assets(assets: &[Asset]) -> Vec<Category> {
    let mut ids = BTreeSet::new();
    for asset in assets {
        ids.insert(asset.category_id);
    }
    ids.into_iter()
        .map(|id| Category {
            id,
            name: format!("Category {id}"),
            parent_category_id: None,
        })
        .collect()
}

#[derive(Clone)]
pub struct HttpApiClient {
    base_url: String,
    agent: ureq::Agent,
}

impl HttpApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(8))
            .build();
        Self {
            base_url: base_url.into(),
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }
}

pub fn timeline_url(
    base_url: &str,
    asset_tag: &str,
    cursor: Option<&str>,
) -> Result<String, ApiError> {
    let mut url = parse_base_url(base_url)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| ApiError::invalid("base url cannot be a base-only URL"))?;
        segments.push("assets");
        segments.push(asset_tag);
        segments.push("events");
    }
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("limit", "100");
        if let Some(c) = cursor {
            query.append_pair("cursor", c);
        }
    }
    Ok(url.into())
}

pub fn event_type_url(base_url: &str, event_type_id: &str) -> Result<String, ApiError> {
    let mut url = parse_base_url(base_url)?;
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| ApiError::invalid("base url cannot be a base-only URL"))?;
    segments.push("event-types");
    segments.push(event_type_id);
    drop(segments);
    Ok(url.into())
}

pub fn apply_event_url(base_url: &str, asset_tag: &str) -> Result<String, ApiError> {
    let mut url = parse_base_url(base_url)?;
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| ApiError::invalid("base url cannot be a base-only URL"))?;
    segments.push("assets");
    segments.push(asset_tag);
    segments.push("events");
    drop(segments);
    Ok(url.into())
}

fn parse_base_url(base_url: &str) -> Result<Url, ApiError> {
    Url::parse(base_url).map_err(|err| ApiError::invalid(&format!("invalid base url: {err}")))
}

impl ApiClient for HttpApiClient {
    fn fetch_categories(&self) -> Result<Vec<Category>, ApiError> {
        #[derive(Deserialize)]
        struct CategoriesListWire {
            items: Vec<CategoryWire>,
        }

        let request = self.agent.get(&self.url("/categories"));
        match request.call() {
            Ok(resp) => {
                let wire: CategoriesListWire = parse_json_response(resp)?;
                Ok(wire.items.into_iter().map(Into::into).collect())
            }
            Err(ureq::Error::Status(404, _)) => Ok(Vec::new()),
            Err(err) => Err(http_error("fetch categories", err)),
        }
    }

    fn fetch_assets(&self) -> Result<Vec<Asset>, ApiError> {
        #[derive(Deserialize)]
        struct AssetsListWire {
            items: Vec<AssetWire>,
        }

        let resp = self
            .agent
            .get(&self.url("/assets"))
            .call()
            .map_err(|err| http_error("fetch assets", err))?;
        let wire: AssetsListWire = parse_json_response(resp)?;
        Ok(wire.items.into_iter().map(Into::into).collect())
    }

    fn fetch_timeline_page(
        &self,
        asset_tag: &str,
        cursor: Option<&str>,
    ) -> Result<TimelinePage, ApiError> {
        #[derive(Deserialize)]
        struct TimelineWire {
            items: Vec<AssetEventWire>,
            next_cursor: Option<String>,
        }

        let url = timeline_url(&self.base_url, asset_tag, cursor)?;
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|err| http_error("fetch events", err))?;
        let wire: TimelineWire = parse_json_response(resp)?;
        Ok(TimelinePage {
            items: wire.items.into_iter().map(Into::into).collect(),
            next_cursor: wire.next_cursor,
        })
    }

    fn fetch_event_type_schema(&self, event_type_id: &str) -> Result<EventTypeSchema, ApiError> {
        let url = event_type_url(&self.base_url, event_type_id)?;
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|err| http_error("fetch event type", err))?;
        let wire: EventTypeWire = parse_json_response(resp)?;

        if let Some(inputs) = wire.inputs {
            return Ok(EventTypeSchema {
                event_type_id: wire.event_type_id,
                inputs: inputs.into_iter().map(Into::into).collect(),
            });
        }

        let mut seen = BTreeSet::new();
        let mut inferred = Vec::new();
        for m in wire.mutations {
            if let Some(input_key) = m.input_key {
                if seen.insert(input_key.clone()) {
                    inferred.push(EventInputSpec {
                        label: input_key.clone(),
                        input_key,
                        kind: EventInputSpecKind::Text,
                    });
                }
            }
        }
        Ok(EventTypeSchema {
            event_type_id: wire.event_type_id,
            inputs: inferred,
        })
    }

    fn apply_event(
        &self,
        asset_tag: &str,
        event_type_id: &str,
        payload: Map<String, Value>,
        idempotency_key: &str,
    ) -> Result<AppliedEvent, ApiError> {
        #[derive(Serialize)]
        struct ApplyRequest {
            event_type_id: String,
            payload: Map<String, Value>,
        }

        let body = ApplyRequest {
            event_type_id: event_type_id.to_string(),
            payload,
        };
        let url = apply_event_url(&self.base_url, asset_tag)?;
        let resp = self
            .agent
            .post(&url)
            .set("content-type", "application/json")
            .set("Idempotency-Key", idempotency_key)
            .send_string(
                &serde_json::to_string(&body).map_err(|err| ApiError::Invalid(err.to_string()))?,
            )
            .map_err(|err| http_error("apply event", err))?;
        let wire: AppliedEventWire = parse_json_response(resp)?;
        Ok(wire.into())
    }
}

fn parse_json_response<T: for<'de> Deserialize<'de>>(resp: ureq::Response) -> Result<T, ApiError> {
    use std::io::Read;

    let mut text = String::new();
    let mut reader = resp.into_reader();
    reader
        .read_to_string(&mut text)
        .map_err(|err| ApiError::Http(err.to_string()))?;
    serde_json::from_str::<T>(&text).map_err(|err| ApiError::Invalid(err.to_string()))
}

fn http_error(step: &str, err: ureq::Error) -> ApiError {
    match err {
        ureq::Error::Status(status, response) => {
            let mut body = String::new();
            let mut reader = response.into_reader();
            let _ = std::io::Read::read_to_string(&mut reader, &mut body);
            ApiError::Http(format!("{step} failed with status {status}: {body}"))
        }
        other => ApiError::Http(format!("{step} failed: {other}")),
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CategoryWire {
    id: i64,
    name: String,
    #[serde(default)]
    parent_category_id: Option<i64>,
}

impl From<CategoryWire> for Category {
    fn from(value: CategoryWire) -> Self {
        Self {
            id: value.id,
            name: value.name,
            parent_category_id: value.parent_category_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AssetWire {
    id: i64,
    category_id: i64,
    asset_tag: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    deleted_at: Option<String>,
}

impl From<AssetWire> for Asset {
    fn from(value: AssetWire) -> Self {
        Self {
            id: value.id,
            category_id: value.category_id,
            asset_tag: value.asset_tag,
            display_name: value.display_name,
            deleted_at: value.deleted_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AssetEventWire {
    event_id: i64,
    event_type_id: String,
    event_type_version: i64,
    payload: Value,
    timestamp: String,
}

impl From<AssetEventWire> for AssetEvent {
    fn from(value: AssetEventWire) -> Self {
        Self {
            event_id: value.event_id,
            event_type_id: value.event_type_id,
            event_type_version: value.event_type_version,
            payload: value.payload,
            timestamp: value.timestamp,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct EventTypeMutationWire {
    #[serde(default)]
    input_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EventTypeWire {
    event_type_id: String,
    #[serde(default)]
    mutations: Vec<EventTypeMutationWire>,
    #[serde(default)]
    inputs: Option<Vec<EventInputWire>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EventInputWire {
    Text {
        input_key: String,
        label: String,
    },
    Integer {
        input_key: String,
        label: String,
    },
    Decimal {
        input_key: String,
        label: String,
    },
    Boolean {
        input_key: String,
        label: String,
    },
    Enum {
        input_key: String,
        label: String,
        options: Vec<EnumOptionWire>,
    },
    ExternalEntity {
        input_key: String,
        label: String,
        options: Vec<ExternalEntityOptionWire>,
    },
}

impl From<EventInputWire> for EventInputSpec {
    fn from(value: EventInputWire) -> Self {
        match value {
            EventInputWire::Text { input_key, label } => Self {
                input_key,
                label,
                kind: EventInputSpecKind::Text,
            },
            EventInputWire::Integer { input_key, label } => Self {
                input_key,
                label,
                kind: EventInputSpecKind::Integer,
            },
            EventInputWire::Decimal { input_key, label } => Self {
                input_key,
                label,
                kind: EventInputSpecKind::Decimal,
            },
            EventInputWire::Boolean { input_key, label } => Self {
                input_key,
                label,
                kind: EventInputSpecKind::Boolean,
            },
            EventInputWire::Enum {
                input_key,
                label,
                options,
            } => Self {
                input_key,
                label,
                kind: EventInputSpecKind::Enum(options.into_iter().map(Into::into).collect()),
            },
            EventInputWire::ExternalEntity {
                input_key,
                label,
                options,
            } => Self {
                input_key,
                label,
                kind: EventInputSpecKind::ExternalEntity(
                    options.into_iter().map(Into::into).collect(),
                ),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct EnumOptionWire {
    option_key: String,
    display_name: String,
}

impl From<EnumOptionWire> for EnumOption {
    fn from(value: EnumOptionWire) -> Self {
        Self {
            option_key: value.option_key,
            display_name: value.display_name,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ExternalEntityOptionWire {
    id: i64,
    display_name: String,
}

impl From<ExternalEntityOptionWire> for ExternalEntityOption {
    fn from(value: ExternalEntityOptionWire) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AppliedEventWire {
    event_id: i64,
    event_type_id: String,
    event_type_version: i64,
    payload: Value,
    timestamp: String,
}

impl From<AppliedEventWire> for AppliedEvent {
    fn from(value: AppliedEventWire) -> Self {
        Self {
            event_id: value.event_id,
            event_type_id: value.event_type_id,
            event_type_version: value.event_type_version,
            payload: value.payload,
            timestamp: value.timestamp,
        }
    }
}

pub type DefaultController = GuiController<HttpApiClient>;

pub fn build_default_controller(base_url: &str) -> DefaultController {
    GuiController::new(
        HttpApiClient::new(base_url.to_string()),
        GuiState::default(),
    )
}

pub fn grouped_assets_by_category(assets: &[Asset]) -> BTreeMap<i64, Vec<Asset>> {
    let mut out: BTreeMap<i64, Vec<Asset>> = BTreeMap::new();
    for asset in assets {
        out.entry(asset.category_id)
            .or_default()
            .push(asset.clone());
    }
    out
}
