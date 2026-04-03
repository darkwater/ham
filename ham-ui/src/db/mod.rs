use egui::Widget;
use rusty_money::{Money, iso::Currency};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AssetDb {
    pub assets: Vec<Asset>,
    pub categories: Vec<Category>,
    pub fields: Vec<Field>,
}

impl AssetDb {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn category(&self, category_id: i64) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == category_id)
    }

    pub fn next_asset_id(&self) -> i64 {
        self.assets.iter().map(|a| a.id).max().unwrap_or(0) + 1
    }

    pub fn next_category_id(&self) -> i64 {
        self.categories.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    pub fn next_field_id(&self) -> i64 {
        self.fields.iter().map(|f| f.id).max().unwrap_or(0) + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: i64,
    pub category_id: i64,
    pub display_name: String,
    pub fields: Vec<AssetField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetField {
    pub field_id: i64,
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
    pub id: i64,
    pub display_name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: i64,
    pub display_name: String,
    pub field_type: FieldType,
}
