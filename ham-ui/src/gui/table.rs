use egui::{Popup, PopupCloseBehavior, Sense};
use egui_table::{HeaderCellInfo, HeaderRow, Table, TableDelegate};

use crate::db::{Asset, AssetDb, FieldId};

pub struct AssetTable<'a> {
    pub db: &'a AssetDb,
    pub columns: &'a mut Vec<AssetColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetColumn {
    Tag,
    Category,
    DisplayName,
    Field(FieldId),
}

impl AssetColumn {
    pub const BASE: &[AssetColumn] =
        &[AssetColumn::Tag, AssetColumn::Category, AssetColumn::DisplayName];

    pub fn width(&self) -> f32 {
        match self {
            AssetColumn::Tag => 80.0,
            AssetColumn::Category => 100.0,
            AssetColumn::DisplayName => 200.0,
            AssetColumn::Field(_) => 150.0,
        }
    }

    fn header(&self, db: &AssetDb) -> String {
        match self {
            AssetColumn::Tag => "Tag".to_string(),
            AssetColumn::Category => "Category".to_string(),
            AssetColumn::DisplayName => "Display Name".to_string(),
            AssetColumn::Field(field_id) => {
                if let Some(field) = db.fields.iter().find(|f| f.id == *field_id) {
                    field.display_name.clone()
                } else {
                    format!("Unknown Field ({:?})", field_id)
                }
            }
        }
    }

    #[expect(clippy::unused_self)]
    pub fn frame(&self) -> egui::Frame {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 3))
    }

    pub fn contents(&self, ui: &mut egui::Ui, db: &AssetDb, asset: &Asset) {
        match self {
            AssetColumn::Tag => {
                ui.label(db.format_asset_tag(asset.id));
            }
            AssetColumn::Category => {
                ui.label(
                    db.category(asset.category_id)
                        .map(|c| c.display_name.clone())
                        .unwrap_or("-".to_string()),
                );
            }
            AssetColumn::DisplayName => {
                ui.label(&asset.display_name);
            }
            AssetColumn::Field(field_id) => {
                if let Some(field) = asset.fields.iter().find(|f| f.field_id == *field_id) {
                    ui.add(&field.value);
                } else {
                    ui.label("-");
                }
            }
        }
    }
}

impl<'a> AssetTable<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        Table::new()
            .columns(
                self.columns
                    .iter()
                    .map(|col| {
                        egui_table::Column::new(col.width())
                            .id(egui::Id::new(col))
                            .resizable(false)
                    })
                    .collect::<Vec<_>>(),
            )
            .num_sticky_cols(1)
            .headers([HeaderRow { height: 24.0, groups: vec![] }])
            .num_rows(self.db.assets.len() as u64)
            .show(ui, self);
    }
}

impl TableDelegate for AssetTable<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        let rect = ui.clip_rect();
        ui.set_clip_rect(rect.expand(2.));

        ui.painter().line_segment(
            [rect.right_top(), rect.right_bottom()],
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        ui.set_clip_rect(rect);

        let res = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let Some(column) = &self.columns.get(cell.col_range.start) else {
                        return;
                    };

                    ui.label(column.header(self.db));
                });
            })
            .response
            .interact(Sense::click());

        Popup::context_menu(&res)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.add_enabled_ui(false, |ui| {
                    ui.checkbox(&mut true, "Tag");
                    ui.checkbox(&mut true, "Category");
                    ui.checkbox(&mut true, "Display Name");
                });

                for field in &self.db.fields {
                    let mut visible = self.columns.contains(&AssetColumn::Field(field.id));

                    if ui.checkbox(&mut visible, &field.display_name).changed() {
                        if visible {
                            self.columns.push(AssetColumn::Field(field.id));
                        } else {
                            self.columns
                                .retain(|col| *col != AssetColumn::Field(field.id));
                        }
                    }
                }
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let Some(asset) = self.db.assets.get(cell.row_nr as usize) else {
            return;
        };

        let Some(column) = self.columns.get(cell.col_nr) else {
            return;
        };

        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            column.frame().show(ui, |ui| {
                column.contents(ui, self.db, asset);
            });
        });
    }

    fn row_ui(&mut self, ui: &mut egui::Ui, row_nr: u64) {
        let odd_row = row_nr % 2 == 1;

        if odd_row {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }
    }

    // fn default_row_height(&self) -> f32 {
    //     28.
    // }
}
