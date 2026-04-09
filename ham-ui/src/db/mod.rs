use core::default::Default;

use egui::Widget;
use rusty_money::{Money, iso::Currency};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetDb {
    pub assets: Vec<Asset>,
    pub categories: Vec<Category>,
    pub fields: Vec<Field>,
    pub settings: Settings,
}

impl AssetDb {
    pub fn format_asset_tag(&self, asset_id: AssetId) -> String {
        format!(
            "{}{:0width$}",
            self.settings.tag_prefix,
            asset_id.0,
            width = self.settings.tag_digits,
        )
    }

    pub fn asset_mut(&mut self, id: AssetId) -> Option<&mut Asset> {
        self.assets.iter_mut().find(|a| a.id == id)
    }

    pub fn create_asset(&mut self) -> AssetId {
        let id = self.next_asset_id();

        self.assets.push(Asset {
            id,
            category_id: CategoryId(1),
            display_name: format!("New Asset {}", id.0),
            fields: Vec::new(),
        });

        id
    }

    pub fn category(&self, category_id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == category_id)
    }

    pub fn next_asset_id(&self) -> AssetId {
        AssetId(self.assets.iter().map(|a| a.id.0).max().unwrap_or(0) + 1)
    }

    pub fn next_category_id(&self) -> CategoryId {
        CategoryId(self.categories.iter().map(|c| c.id.0).max().unwrap_or(0) + 1)
    }

    pub fn next_field_id(&self) -> FieldId {
        FieldId(self.fields.iter().map(|f| f.id.0).max().unwrap_or(0) + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CategoryId(i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FieldId(i64);

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

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    #[default]
    String,
    Int,
    Float,
    Money,
    Boolean,
    DateTime(DateTimePrecision),
}

impl Widget for &mut FieldType {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::ComboBox::from_id_salt(ui.auto_id_with("field type"))
            .selected_text(format!("{:?}", self))
            .show_ui(ui, |ui| {
                ui.selectable_value(self, FieldType::String, "String");
                ui.selectable_value(self, FieldType::Int, "Int");
                ui.selectable_value(self, FieldType::Float, "Float");
                ui.selectable_value(self, FieldType::Money, "Money");
                ui.selectable_value(self, FieldType::Boolean, "Boolean");
                ui.menu_button("DateTime", |ui| {
                    ui.selectable_value(self, FieldType::DateTime(DateTimePrecision::Year), "Year");
                    ui.selectable_value(
                        self,
                        FieldType::DateTime(DateTimePrecision::Month),
                        "Month",
                    );
                    ui.selectable_value(self, FieldType::DateTime(DateTimePrecision::Day), "Day");
                    ui.selectable_value(self, FieldType::DateTime(DateTimePrecision::Hour), "Hour");
                    ui.selectable_value(
                        self,
                        FieldType::DateTime(DateTimePrecision::Minute),
                        "Minute",
                    );
                    ui.selectable_value(
                        self,
                        FieldType::DateTime(DateTimePrecision::Second),
                        "Second",
                    );
                });
            })
            .response
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldValue {
    String(String),
    Int(i64),
    Float(f64),
    Money(Money<'static, Currency>),
    Boolean(bool),
    DateTime {
        date: chrono::DateTime<chrono::Utc>,
        precision: DateTimePrecision,
    },
}

impl FieldValue {
    pub fn field_type(&self) -> FieldType {
        match self {
            FieldValue::String(_) => FieldType::String,
            FieldValue::Int(_) => FieldType::Int,
            FieldValue::Float(_) => FieldType::Float,
            FieldValue::Money(_) => FieldType::Money,
            FieldValue::Boolean(_) => FieldType::Boolean,
            FieldValue::DateTime { precision, .. } => FieldType::DateTime(*precision),
        }
    }
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

impl Widget for &FieldValue {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        match self {
            FieldValue::String(s) => ui.label(s),
            FieldValue::Int(i) => ui.label(i.to_string()),
            FieldValue::Float(f) => ui.label(f.to_string()),
            FieldValue::Money(m) => ui.label(m.to_string()),
            FieldValue::Boolean(b) => ui.checkbox(&mut b.clone(), ""),
            FieldValue::DateTime { date, precision } => {
                let formatted = match precision {
                    DateTimePrecision::Year => date.format("%Y").to_string(),
                    DateTimePrecision::Month => date.format("%Y-%m").to_string(),
                    DateTimePrecision::Day => date.format("%Y-%m-%d").to_string(),
                    DateTimePrecision::Hour => date.format("%Y-%m-%d %H:00").to_string(),
                    DateTimePrecision::Minute => date.format("%Y-%m-%d %H:%M").to_string(),
                    DateTimePrecision::Second => date.format("%Y-%m-%d %H:%M:%S").to_string(),
                };
                ui.label(formatted)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub display_name: String,
    pub parent_id: Option<CategoryId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: FieldId,
    pub display_name: String,
    pub field_type: FieldType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub tag_prefix: String,
    pub tag_digits: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tag_prefix: "A".to_string(),
            tag_digits: 4,
        }
    }
}
