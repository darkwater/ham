use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, Connection, ErrorCode};
use serde_json::Value;
use thiserror::Error;

use crate::db::repo_tag_generator;

#[derive(Debug, Clone)]
pub struct CreatedAsset {
    pub id: i64,
    pub category_id: i64,
    pub asset_tag: String,
}

#[derive(Debug, Error)]
pub enum AssetCreateError {
    #[error("asset tag `{0}` already exists")]
    DuplicateAssetTag(String),
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct AssetSearchFilter {
    pub field: String,
    pub op: String,
    pub value: Option<Value>,
    pub values: Option<Vec<Value>>,
    pub include_subtree: bool,
}

#[derive(Debug, Clone)]
pub struct AssetSearchFilterGroup {
    pub filters: Vec<AssetSearchFilter>,
}

#[derive(Debug, Clone)]
pub struct AssetSearchSort {
    pub field: String,
    pub direction: String,
}

#[derive(Debug, Clone)]
pub struct AssetSearchQuery {
    pub filters: Vec<AssetSearchFilter>,
    pub or_groups: Vec<AssetSearchFilterGroup>,
    pub sort: Vec<AssetSearchSort>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    pub include_total_estimate: bool,
}

#[derive(Debug, Clone)]
pub struct AssetSearchItem {
    pub id: i64,
    pub category_id: i64,
    pub asset_tag: String,
    pub display_name: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssetSearchPage {
    pub items: Vec<AssetSearchItem>,
    pub next_cursor: Option<String>,
    pub total_estimate: Option<i64>,
}

#[derive(Debug, Error)]
pub enum AssetSearchError {
    #[error("invalid search request: {0}")]
    InvalidRequest(String),
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
struct AssetRecord {
    id: i64,
    category_id: i64,
    asset_tag: String,
    display_name: Option<String>,
    deleted_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone)]
struct TagDefinitionMeta {
    value_type: String,
    external_entity_type_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct TagValueRecord {
    value_json: Value,
    value_type: String,
    external_entity_id: Option<i64>,
    external_entity_type_id: Option<i64>,
}

pub fn search_assets(
    conn: &Connection,
    query: &AssetSearchQuery,
) -> Result<AssetSearchPage, AssetSearchError> {
    let mut assets = load_assets(conn)?;
    let tag_defs = load_tag_definitions(conn)?;
    let tag_values = load_tag_values(conn)?;
    let category_children = load_category_children(conn)?;

    validate_filters(query, &tag_defs)?;
    let sort_spec = normalize_and_validate_sorts(query, &tag_defs)?;

    let mut filtered = Vec::new();
    for asset in assets {
        if matches_asset(&asset, query, &tag_defs, &tag_values, &category_children)? {
            filtered.push(asset);
        }
    }
    assets = filtered;

    assets.sort_by(|left, right| compare_assets(left, right, &sort_spec, &tag_defs, &tag_values));

    let start = match query.cursor.as_deref() {
        Some(raw) => raw
            .parse::<usize>()
            .map_err(|_| AssetSearchError::InvalidRequest("invalid cursor".to_string()))?,
        None => 0,
    };
    if start > assets.len() {
        return Err(AssetSearchError::InvalidRequest(
            "invalid cursor".to_string(),
        ));
    }

    let limit = query.limit.unwrap_or(50);
    let end = start.saturating_add(limit).min(assets.len());
    let page_items = assets[start..end]
        .iter()
        .map(|asset| AssetSearchItem {
            id: asset.id,
            category_id: asset.category_id,
            asset_tag: asset.asset_tag.clone(),
            display_name: asset.display_name.clone(),
            deleted_at: asset.deleted_at.clone(),
        })
        .collect::<Vec<_>>();

    Ok(AssetSearchPage {
        items: page_items,
        next_cursor: if end < assets.len() {
            Some(end.to_string())
        } else {
            None
        },
        total_estimate: if query.include_total_estimate {
            Some(assets.len() as i64)
        } else {
            None
        },
    })
}

fn validate_filters(
    query: &AssetSearchQuery,
    tag_defs: &HashMap<String, TagDefinitionMeta>,
) -> Result<(), AssetSearchError> {
    for filter in &query.filters {
        validate_filter(filter, tag_defs)?;
    }
    for group in &query.or_groups {
        for filter in &group.filters {
            validate_filter(filter, tag_defs)?;
        }
    }
    Ok(())
}

fn validate_filter(
    filter: &AssetSearchFilter,
    tag_defs: &HashMap<String, TagDefinitionMeta>,
) -> Result<(), AssetSearchError> {
    let op = filter.op.to_ascii_lowercase();
    if filter.field == "text" {
        if op != "contains" {
            return Err(AssetSearchError::InvalidRequest(
                "text predicate only supports contains".to_string(),
            ));
        }
        let _ = required_text_value(filter, "contains")?;
        return Ok(());
    }

    if let Some(type_id) = parse_external_entity_type_id(&filter.field) {
        let exists = tag_defs.values().any(|meta| {
            meta.value_type == "external_entity" && meta.external_entity_type_id == Some(type_id)
        });
        if !exists {
            return Err(AssetSearchError::InvalidRequest(format!(
                "unknown external entity type id `{type_id}`"
            )));
        }
        return match op.as_str() {
            "eq" => {
                let _ = required_integer_value(filter, "eq")?;
                Ok(())
            }
            "is_null" | "is_not_null" => Ok(()),
            _ => Err(AssetSearchError::InvalidRequest(
                "external_entity(type_id) only supports eq/is_null/is_not_null".to_string(),
            )),
        };
    }

    match filter.field.as_str() {
        "id" | "category_id" => validate_numeric_filter(filter),
        "asset_tag" | "display_name" | "deleted_at" | "created_at" => validate_text_filter(filter),
        _ => {
            let Some(meta) = tag_defs.get(&filter.field) else {
                return Err(AssetSearchError::InvalidRequest(format!(
                    "unknown filter field `{}`",
                    filter.field
                )));
            };
            match meta.value_type.as_str() {
                "boolean" => match op.as_str() {
                    "eq" => {
                        let _ = required_bool_value(filter)?;
                        Ok(())
                    }
                    "is_null" | "is_not_null" => Ok(()),
                    _ => Err(AssetSearchError::InvalidRequest(
                        "boolean filters only support eq/is_null/is_not_null".to_string(),
                    )),
                },
                "integer" | "decimal" | "money" | "external_entity" => {
                    validate_numeric_filter(filter)
                }
                "date" | "datetime" | "text" | "enum" | "ipv4" | "url" | "mac_address" => {
                    validate_text_filter(filter)
                }
                _ => validate_text_filter(filter),
            }
        }
    }
}

fn validate_numeric_filter(filter: &AssetSearchFilter) -> Result<(), AssetSearchError> {
    let op = filter.op.to_ascii_lowercase();
    match op.as_str() {
        "eq" | "lt" | "lte" | "gt" | "gte" => {
            let _ = required_numeric_value(filter, &op)?;
            Ok(())
        }
        "between" => {
            let _ = required_numeric_between_values(filter)?;
            Ok(())
        }
        "is_null" | "is_not_null" => Ok(()),
        _ => Err(AssetSearchError::InvalidRequest(format!(
            "unsupported operator `{}`",
            filter.op
        ))),
    }
}

fn validate_text_filter(filter: &AssetSearchFilter) -> Result<(), AssetSearchError> {
    let op = filter.op.to_ascii_lowercase();
    match op.as_str() {
        "eq" | "contains" | "lt" | "lte" | "gt" | "gte" => {
            let _ = required_text_value(filter, &op)?;
            Ok(())
        }
        "between" => {
            let _ = required_text_between_values(filter)?;
            Ok(())
        }
        "is_null" | "is_not_null" => Ok(()),
        _ => Err(AssetSearchError::InvalidRequest(format!(
            "unsupported operator `{}`",
            filter.op
        ))),
    }
}

fn normalize_and_validate_sorts(
    query: &AssetSearchQuery,
    tag_defs: &HashMap<String, TagDefinitionMeta>,
) -> Result<Vec<AssetSearchSort>, AssetSearchError> {
    let mut sort_spec = query.sort.clone();
    if sort_spec.is_empty() {
        sort_spec.push(AssetSearchSort {
            field: "asset_tag".to_string(),
            direction: "asc".to_string(),
        });
    }

    for sort in &sort_spec {
        let direction = sort.direction.to_ascii_lowercase();
        if direction != "asc" && direction != "desc" {
            return Err(AssetSearchError::InvalidRequest(format!(
                "invalid sort direction `{}`",
                sort.direction
            )));
        }

        let known_builtin = matches!(
            sort.field.as_str(),
            "id" | "category_id" | "asset_tag" | "display_name" | "created_at"
        );
        if !known_builtin && !tag_defs.contains_key(&sort.field) {
            return Err(AssetSearchError::InvalidRequest(format!(
                "invalid sort field `{}`",
                sort.field
            )));
        }
    }

    if !sort_spec
        .iter()
        .any(|s| s.field.eq_ignore_ascii_case("asset_tag"))
    {
        sort_spec.push(AssetSearchSort {
            field: "asset_tag".to_string(),
            direction: "asc".to_string(),
        });
    }

    Ok(sort_spec)
}

pub fn create_asset(
    conn: &mut Connection,
    category_id: i64,
    manual_asset_tag: Option<&str>,
) -> Result<CreatedAsset, AssetCreateError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    if let Some(tag) = manual_asset_tag {
        let insert_result = tx.execute(
            "INSERT INTO assets (category_id, asset_tag) VALUES (?1, ?2)",
            params![category_id, tag],
        );

        if let Err(err) = insert_result {
            if is_duplicate_asset_tag_constraint(&err) {
                return Err(AssetCreateError::DuplicateAssetTag(tag.to_string()));
            }
            return Err(AssetCreateError::Sql(err));
        }

        let id = tx.last_insert_rowid();
        tx.commit()?;
        return Ok(CreatedAsset {
            id,
            category_id,
            asset_tag: tag.to_string(),
        });
    }

    let settings = repo_tag_generator::load_settings(&tx)?;
    let mut next_value = repo_tag_generator::load_global_next_value(&tx)?;

    loop {
        let candidate = repo_tag_generator::format_tag(&settings, next_value);

        let insert_result = tx.execute(
            "INSERT INTO assets (category_id, asset_tag) VALUES (?1, ?2)",
            params![category_id, candidate.as_str()],
        );

        if let Err(err) = insert_result {
            if is_duplicate_asset_tag_constraint(&err) {
                next_value += 1;
                continue;
            }
            return Err(AssetCreateError::Sql(err));
        }

        let id = tx.last_insert_rowid();
        repo_tag_generator::persist_global_next_value(&tx, next_value + 1)?;
        tx.commit()?;

        return Ok(CreatedAsset {
            id,
            category_id,
            asset_tag: candidate,
        });
    }
}

fn is_duplicate_asset_tag_constraint(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(code, message) => {
            code.code == ErrorCode::ConstraintViolation
                && message
                    .as_deref()
                    .is_some_and(|msg| msg.contains("UNIQUE constraint failed: assets.asset_tag"))
        }
        _ => false,
    }
}

fn load_assets(conn: &Connection) -> Result<Vec<AssetRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "
        SELECT id, category_id, asset_tag, display_name, deleted_at, created_at
        FROM assets
        WHERE deleted_at IS NULL
        ",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(AssetRecord {
            id: row.get(0)?,
            category_id: row.get(1)?,
            asset_tag: row.get(2)?,
            display_name: row.get(3)?,
            deleted_at: row.get(4)?,
            created_at: row.get(5)?,
        });
    }
    Ok(out)
}

fn load_tag_definitions(
    conn: &Connection,
) -> Result<HashMap<String, TagDefinitionMeta>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT tag_key, value_type, external_entity_type_id FROM tag_definitions")?;
    let mut rows = stmt.query([])?;
    let mut out = HashMap::new();
    while let Some(row) = rows.next()? {
        let tag_key: String = row.get(0)?;
        out.insert(
            tag_key,
            TagDefinitionMeta {
                value_type: row.get(1)?,
                external_entity_type_id: row.get(2)?,
            },
        );
    }
    Ok(out)
}

fn load_tag_values(
    conn: &Connection,
) -> Result<HashMap<i64, HashMap<String, TagValueRecord>>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "
        SELECT
            acv.asset_id,
            td.tag_key,
            td.value_type,
            td.external_entity_type_id,
            acv.value_json,
            acv.external_entity_id
        FROM asset_current_tag_values acv
        JOIN tag_definitions td ON td.id = acv.tag_definition_id
        ",
    )?;
    let mut rows = stmt.query([])?;

    let mut out: HashMap<i64, HashMap<String, TagValueRecord>> = HashMap::new();
    while let Some(row) = rows.next()? {
        let asset_id: i64 = row.get(0)?;
        let tag_key: String = row.get(1)?;
        let value_type: String = row.get(2)?;
        let external_entity_type_id: Option<i64> = row.get(3)?;
        let value_json_str: String = row.get(4)?;
        let external_entity_id: Option<i64> = row.get(5)?;

        let value_json = serde_json::from_str(&value_json_str).unwrap_or(Value::Null);
        out.entry(asset_id).or_default().insert(
            tag_key,
            TagValueRecord {
                value_json,
                value_type,
                external_entity_id,
                external_entity_type_id,
            },
        );
    }

    Ok(out)
}

fn load_category_children(conn: &Connection) -> Result<HashMap<i64, Vec<i64>>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, parent_category_id FROM categories")?;
    let mut rows = stmt.query([])?;
    let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let parent: Option<i64> = row.get(1)?;
        if let Some(parent_id) = parent {
            children.entry(parent_id).or_default().push(id);
        }
    }
    Ok(children)
}

fn matches_asset(
    asset: &AssetRecord,
    query: &AssetSearchQuery,
    tag_defs: &HashMap<String, TagDefinitionMeta>,
    tag_values: &HashMap<i64, HashMap<String, TagValueRecord>>,
    category_children: &HashMap<i64, Vec<i64>>,
) -> Result<bool, AssetSearchError> {
    for filter in &query.filters {
        if !matches_filter(asset, filter, tag_defs, tag_values, category_children)? {
            return Ok(false);
        }
    }

    if query.or_groups.is_empty() {
        return Ok(true);
    }

    for group in &query.or_groups {
        let mut group_match = true;
        for filter in &group.filters {
            if !matches_filter(asset, filter, tag_defs, tag_values, category_children)? {
                group_match = false;
                break;
            }
        }
        if group_match {
            return Ok(true);
        }
    }

    Ok(false)
}

fn matches_filter(
    asset: &AssetRecord,
    filter: &AssetSearchFilter,
    tag_defs: &HashMap<String, TagDefinitionMeta>,
    tag_values: &HashMap<i64, HashMap<String, TagValueRecord>>,
    category_children: &HashMap<i64, Vec<i64>>,
) -> Result<bool, AssetSearchError> {
    let op = filter.op.to_ascii_lowercase();

    if filter.field == "text" {
        return match op.as_str() {
            "contains" => {
                let needle = filter
                    .value
                    .as_ref()
                    .and_then(value_as_string)
                    .unwrap_or_default()
                    .to_ascii_lowercase();

                if needle.is_empty() {
                    return Ok(true);
                }

                let mut haystacks = vec![asset.asset_tag.to_ascii_lowercase()];
                if let Some(display_name) = &asset.display_name {
                    haystacks.push(display_name.to_ascii_lowercase());
                }
                if let Some(values) = tag_values.get(&asset.id) {
                    for value in values.values() {
                        if let Some(text) = value_as_string(&value.value_json) {
                            haystacks.push(text.to_ascii_lowercase());
                        }
                    }
                }

                Ok(haystacks.iter().any(|value| value.contains(&needle)))
            }
            _ => Err(AssetSearchError::InvalidRequest(format!(
                "unsupported op `{}` for text predicate",
                filter.op
            ))),
        };
    }

    if filter.field == "category_id" && filter.include_subtree {
        let Some(category_id) = filter.value.as_ref().and_then(value_as_i64) else {
            return Err(AssetSearchError::InvalidRequest(
                "category subtree filter requires integer value".to_string(),
            ));
        };

        let mut allowed: HashSet<i64> = HashSet::new();
        allowed.insert(category_id);
        let mut queue = VecDeque::from([category_id]);
        while let Some(current) = queue.pop_front() {
            if let Some(children) = category_children.get(&current) {
                for child in children {
                    if allowed.insert(*child) {
                        queue.push_back(*child);
                    }
                }
            }
        }

        return match op.as_str() {
            "eq" => Ok(allowed.contains(&asset.category_id)),
            _ => Err(AssetSearchError::InvalidRequest(
                "category subtree filter only supports eq".to_string(),
            )),
        };
    }

    if let Some(type_id) = parse_external_entity_type_id(&filter.field) {
        return match_external_entity_type_filter(asset, filter, tag_values, type_id);
    }

    match filter.field.as_str() {
        "id" => compare_numeric_or_null(Some(asset.id as f64), &op, filter),
        "asset_tag" => compare_text_or_null(Some(asset.asset_tag.clone()), &op, filter),
        "display_name" => compare_text_or_null(asset.display_name.clone(), &op, filter),
        "category_id" => compare_numeric_or_null(Some(asset.category_id as f64), &op, filter),
        "deleted_at" => compare_text_or_null(asset.deleted_at.clone(), &op, filter),
        "created_at" => compare_text_or_null(Some(asset.created_at.clone()), &op, filter),
        _ => {
            let Some(tag_meta) = tag_defs.get(&filter.field) else {
                return Err(AssetSearchError::InvalidRequest(format!(
                    "unknown filter field `{}`",
                    filter.field
                )));
            };
            let tag_value = tag_values
                .get(&asset.id)
                .and_then(|m| m.get(&filter.field))
                .cloned();
            match tag_value {
                None => compare_missing(&op),
                Some(record) => compare_tag_value(&record, tag_meta, &op, filter),
            }
        }
    }
}

fn compare_missing(op: &str) -> Result<bool, AssetSearchError> {
    match op {
        "is_null" => Ok(true),
        "is_not_null" => Ok(false),
        "eq" | "contains" | "lt" | "lte" | "gt" | "gte" | "between" => Ok(false),
        _ => Err(AssetSearchError::InvalidRequest(format!(
            "unsupported operator `{op}`"
        ))),
    }
}

fn compare_tag_value(
    record: &TagValueRecord,
    tag_meta: &TagDefinitionMeta,
    op: &str,
    filter: &AssetSearchFilter,
) -> Result<bool, AssetSearchError> {
    if op == "is_null" {
        return Ok(record.value_json.is_null());
    }
    if op == "is_not_null" {
        return Ok(!record.value_json.is_null());
    }

    match tag_meta.value_type.as_str() {
        "boolean" => {
            let lhs = record.value_json.as_bool();
            compare_bool(lhs, op, filter)
        }
        "integer" | "decimal" | "money" => {
            let lhs = value_to_f64(&record.value_json);
            compare_numeric_or_null(lhs, op, filter)
        }
        "date" | "datetime" => {
            let lhs = value_as_string(&record.value_json);
            compare_text_or_null(lhs, op, filter)
        }
        "enum" => {
            let lhs = value_as_string(&record.value_json);
            compare_text_or_null(lhs, op, filter)
        }
        "external_entity" => {
            let lhs = record.external_entity_id.map(|v| v as f64);
            compare_numeric_or_null(lhs, op, filter)
        }
        "ipv4" => {
            let lhs = value_as_string(&record.value_json);
            compare_text_or_null(lhs, op, filter)
        }
        _ => {
            let lhs = value_as_string(&record.value_json);
            compare_text_or_null(lhs, op, filter)
        }
    }
}

fn match_external_entity_type_filter(
    asset: &AssetRecord,
    filter: &AssetSearchFilter,
    tag_values: &HashMap<i64, HashMap<String, TagValueRecord>>,
    type_id: i64,
) -> Result<bool, AssetSearchError> {
    let op = filter.op.to_ascii_lowercase();
    if op != "eq" && op != "is_null" && op != "is_not_null" {
        return Err(AssetSearchError::InvalidRequest(
            "external_entity(type_id) only supports eq/is_null/is_not_null".to_string(),
        ));
    }

    let values = tag_values.get(&asset.id);
    let mut matched_any_type = false;
    let mut matched_eq = false;

    if let Some(values) = values {
        for value in values.values() {
            if value.value_type == "external_entity"
                && value.external_entity_type_id == Some(type_id)
            {
                matched_any_type = true;
                if let Some(expected) = filter.value.as_ref().and_then(value_as_i64) {
                    if value.external_entity_id == Some(expected) {
                        matched_eq = true;
                    }
                }
            }
        }
    }

    match op.as_str() {
        "eq" => Ok(matched_eq),
        "is_null" => Ok(!matched_any_type),
        "is_not_null" => Ok(matched_any_type),
        _ => Err(AssetSearchError::InvalidRequest(
            "unsupported operator".to_string(),
        )),
    }
}

fn compare_text_or_null(
    lhs: Option<String>,
    op: &str,
    filter: &AssetSearchFilter,
) -> Result<bool, AssetSearchError> {
    match op {
        "is_null" => Ok(lhs.is_none()),
        "is_not_null" => Ok(lhs.is_some()),
        "eq" => {
            let rhs = required_text_value(filter, "eq")?;
            Ok(lhs == rhs)
        }
        "contains" => {
            let Some(lhs) = lhs else {
                return Ok(false);
            };
            let Some(rhs) = required_text_value(filter, "contains")? else {
                return Ok(false);
            };
            Ok(lhs.to_ascii_lowercase().contains(&rhs.to_ascii_lowercase()))
        }
        "lt" | "lte" | "gt" | "gte" => {
            let Some(lhs) = lhs else {
                return Ok(false);
            };
            let Some(rhs) = required_text_value(filter, op)? else {
                return Ok(false);
            };
            Ok(compare_ord(&lhs, &rhs, op))
        }
        "between" => {
            let Some(lhs) = lhs else {
                return Ok(false);
            };
            let (Some(low), Some(high)) = required_text_between_values(filter)? else {
                return Ok(false);
            };
            Ok(lhs >= low && lhs <= high)
        }
        _ => Err(AssetSearchError::InvalidRequest(format!(
            "unsupported operator `{op}`"
        ))),
    }
}

fn compare_numeric_or_null(
    lhs: Option<f64>,
    op: &str,
    filter: &AssetSearchFilter,
) -> Result<bool, AssetSearchError> {
    match op {
        "is_null" => Ok(lhs.is_none()),
        "is_not_null" => Ok(lhs.is_some()),
        "eq" | "lt" | "lte" | "gt" | "gte" => {
            let Some(lhs) = lhs else {
                return Ok(false);
            };
            let Some(rhs) = required_numeric_value(filter, op)? else {
                return Ok(false);
            };
            Ok(match op {
                "eq" => lhs == rhs,
                "lt" => lhs < rhs,
                "lte" => lhs <= rhs,
                "gt" => lhs > rhs,
                "gte" => lhs >= rhs,
                _ => false,
            })
        }
        "between" => {
            let Some(lhs) = lhs else {
                return Ok(false);
            };
            let (Some(low), Some(high)) = required_numeric_between_values(filter)? else {
                return Ok(false);
            };
            Ok(lhs >= low && lhs <= high)
        }
        _ => Err(AssetSearchError::InvalidRequest(format!(
            "unsupported operator `{op}`"
        ))),
    }
}

fn compare_bool(
    lhs: Option<bool>,
    op: &str,
    filter: &AssetSearchFilter,
) -> Result<bool, AssetSearchError> {
    match op {
        "is_null" => Ok(lhs.is_none()),
        "is_not_null" => Ok(lhs.is_some()),
        "eq" => {
            let rhs = required_bool_value(filter)?;
            Ok(lhs == rhs)
        }
        _ => Err(AssetSearchError::InvalidRequest(
            "boolean filters only support eq/is_null/is_not_null".to_string(),
        )),
    }
}

fn compare_ord(lhs: &str, rhs: &str, op: &str) -> bool {
    match op {
        "lt" => lhs < rhs,
        "lte" => lhs <= rhs,
        "gt" => lhs > rhs,
        "gte" => lhs >= rhs,
        _ => false,
    }
}

fn required_text_value(
    filter: &AssetSearchFilter,
    op_name: &str,
) -> Result<Option<String>, AssetSearchError> {
    let Some(value) = filter.value.as_ref() else {
        return Err(AssetSearchError::InvalidRequest(format!(
            "operator `{op_name}` requires `value`"
        )));
    };
    let Some(text) = value_as_string(value) else {
        return Err(AssetSearchError::InvalidRequest(format!(
            "operator `{op_name}` requires text-compatible `value`"
        )));
    };
    Ok(Some(text))
}

fn required_text_between_values(
    filter: &AssetSearchFilter,
) -> Result<(Option<String>, Option<String>), AssetSearchError> {
    let Some(values) = filter.values.as_ref() else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires `values`".to_string(),
        ));
    };
    if values.len() != 2 {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires exactly 2 values".to_string(),
        ));
    }
    let Some(low) = value_as_string(&values[0]) else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires text-compatible bounds".to_string(),
        ));
    };
    let Some(high) = value_as_string(&values[1]) else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires text-compatible bounds".to_string(),
        ));
    };
    Ok((Some(low), Some(high)))
}

fn required_numeric_value(
    filter: &AssetSearchFilter,
    op_name: &str,
) -> Result<Option<f64>, AssetSearchError> {
    let Some(value) = filter.value.as_ref() else {
        return Err(AssetSearchError::InvalidRequest(format!(
            "operator `{op_name}` requires `value`"
        )));
    };
    let Some(number) = value_to_f64(value) else {
        return Err(AssetSearchError::InvalidRequest(format!(
            "operator `{op_name}` requires numeric `value`"
        )));
    };
    Ok(Some(number))
}

fn required_integer_value(
    filter: &AssetSearchFilter,
    op_name: &str,
) -> Result<Option<i64>, AssetSearchError> {
    let Some(value) = filter.value.as_ref() else {
        return Err(AssetSearchError::InvalidRequest(format!(
            "operator `{op_name}` requires `value`"
        )));
    };
    let Some(number) = value_as_i64(value) else {
        return Err(AssetSearchError::InvalidRequest(format!(
            "operator `{op_name}` requires integer `value`"
        )));
    };
    Ok(Some(number))
}

fn required_numeric_between_values(
    filter: &AssetSearchFilter,
) -> Result<(Option<f64>, Option<f64>), AssetSearchError> {
    let Some(values) = filter.values.as_ref() else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires `values`".to_string(),
        ));
    };
    if values.len() != 2 {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires exactly 2 values".to_string(),
        ));
    }
    let Some(low) = value_to_f64(&values[0]) else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires numeric bounds".to_string(),
        ));
    };
    let Some(high) = value_to_f64(&values[1]) else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `between` requires numeric bounds".to_string(),
        ));
    };
    Ok((Some(low), Some(high)))
}

fn required_bool_value(filter: &AssetSearchFilter) -> Result<Option<bool>, AssetSearchError> {
    let Some(value) = filter.value.as_ref() else {
        return Err(AssetSearchError::InvalidRequest(
            "operator `eq` requires `value` for boolean field".to_string(),
        ));
    };
    let Some(boolean) = value.as_bool() else {
        return Err(AssetSearchError::InvalidRequest(
            "boolean eq filter requires boolean `value`".to_string(),
        ));
    };
    Ok(Some(boolean))
}

fn parse_external_entity_type_id(field: &str) -> Option<i64> {
    if !field.starts_with("external_entity(") || !field.ends_with(')') {
        return None;
    }
    let inner = &field[16..field.len() - 1];
    inner.parse::<i64>().ok()
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn compare_option_num(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
    }
}

fn compare_option_text(left: Option<String>, right: Option<String>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a.cmp(&b),
    }
}

fn compare_option_bool(left: Option<bool>, right: Option<bool>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a.cmp(&b),
    }
}

fn compare_sort_field(
    left: &AssetRecord,
    right: &AssetRecord,
    field: &str,
    tag_defs: &HashMap<String, TagDefinitionMeta>,
    tag_values: &HashMap<i64, HashMap<String, TagValueRecord>>,
) -> Ordering {
    match field {
        "id" => compare_option_num(Some(left.id as f64), Some(right.id as f64)),
        "category_id" => compare_option_num(
            Some(left.category_id as f64),
            Some(right.category_id as f64),
        ),
        "asset_tag" => {
            compare_option_text(Some(left.asset_tag.clone()), Some(right.asset_tag.clone()))
        }
        "display_name" => {
            compare_option_text(left.display_name.clone(), right.display_name.clone())
        }
        "created_at" => compare_option_text(
            Some(left.created_at.clone()),
            Some(right.created_at.clone()),
        ),
        tag_key => {
            let meta = tag_defs.get(tag_key);
            let left_value = tag_values.get(&left.id).and_then(|m| m.get(tag_key));
            let right_value = tag_values.get(&right.id).and_then(|m| m.get(tag_key));

            match meta.map(|m| m.value_type.as_str()).unwrap_or("text") {
                "integer" | "decimal" | "money" => compare_option_num(
                    left_value.and_then(|v| value_to_f64(&v.value_json)),
                    right_value.and_then(|v| value_to_f64(&v.value_json)),
                ),
                "external_entity" => compare_option_num(
                    left_value
                        .and_then(|v| v.external_entity_id)
                        .map(|n| n as f64),
                    right_value
                        .and_then(|v| v.external_entity_id)
                        .map(|n| n as f64),
                ),
                "boolean" => compare_option_bool(
                    left_value.and_then(|v| v.value_json.as_bool()),
                    right_value.and_then(|v| v.value_json.as_bool()),
                ),
                "date" | "datetime" | "text" | "enum" | "ipv4" | "url" | "mac_address" => {
                    compare_option_text(
                        left_value.and_then(|v| value_as_string(&v.value_json)),
                        right_value.and_then(|v| value_as_string(&v.value_json)),
                    )
                }
                _ => compare_option_text(
                    left_value.and_then(|v| value_as_string(&v.value_json)),
                    right_value.and_then(|v| value_as_string(&v.value_json)),
                ),
            }
        }
    }
}

fn compare_assets(
    left: &AssetRecord,
    right: &AssetRecord,
    sorts: &[AssetSearchSort],
    tag_defs: &HashMap<String, TagDefinitionMeta>,
    tag_values: &HashMap<i64, HashMap<String, TagValueRecord>>,
) -> Ordering {
    for sort in sorts {
        let direction = sort.direction.to_ascii_lowercase();
        let mut cmp = compare_sort_field(left, right, &sort.field, tag_defs, tag_values);
        if direction == "desc" {
            cmp = cmp.reverse();
        }
        if cmp != Ordering::Equal {
            return cmp;
        }
    }
    Ordering::Equal
}
