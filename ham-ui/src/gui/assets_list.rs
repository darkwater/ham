use core::iter;

use egui::{CursorIcon, Frame, Margin, Sense, TextWrapMode};
use egui_table::{HeaderCellInfo, HeaderRow, Table, TableDelegate};
use ham_shared::{Asset, FieldId};
use serde::{Deserialize, Serialize};

use crate::gui::{ElmCtx, GlobalState, HamPage, Message};

pub struct AssetsList<'a> {
    pub global: &'a GlobalState,
    pub elm: ElmCtx<'a, Message>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AssetColumn {
    Tag,
    DisplayName,
    Category,
    Field(FieldId),
}

impl AssetColumn {
    pub const BASE: &[AssetColumn] =
        &[AssetColumn::Tag, AssetColumn::DisplayName, AssetColumn::Category];

    pub fn width(&self) -> f32 {
        match self {
            AssetColumn::Tag => 80.0,
            AssetColumn::DisplayName => 200.0,
            AssetColumn::Category => 100.0,
            AssetColumn::Field(_) => 150.0,
        }
    }

    fn header(&self, global: &GlobalState) -> String {
        match self {
            AssetColumn::Tag => "Tag".to_string(),
            AssetColumn::DisplayName => "Display Name".to_string(),
            AssetColumn::Category => "Category".to_string(),
            AssetColumn::Field(field_id) => {
                if let Some(field) = global.field(*field_id) {
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

    pub fn contents(&self, ui: &mut egui::Ui, global: &GlobalState, asset: &Asset) {
        match self {
            AssetColumn::Tag => {
                ui.label(global.format_asset_tag(asset.id));
            }
            AssetColumn::DisplayName => {
                ui.label(&asset.display_name);
            }
            AssetColumn::Category => {
                ui.label(
                    global
                        .category(asset.category_id)
                        .map(|c| c.display_name.clone())
                        .unwrap_or("-".to_string()),
                );
            }
            AssetColumn::Field(field_id) => {
                if let Some(field) = asset.fields.iter().find(|f| f.field_id == *field_id) {
                    ui.label(field.value.to_string());
                } else {
                    ui.label("-");
                }
            }
        }
    }
}

impl<'a> AssetsList<'a> {
    pub fn columns(&self) -> &[AssetColumn] {
        &self.global.settings.asset_columns
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("right_panel").show_inside(ui, |ui| {
            for field in &self.global.fields {
                let mut checked = self
                    .global
                    .settings
                    .asset_columns
                    .contains(&AssetColumn::Field(field.id));

                let res = ui.checkbox(&mut checked, &field.display_name);
                if res.changed() {
                    self.elm
                        .send(Message::ToggleFetchAssetField(field.id, checked));
                }
            }

            ui.take_available_space();
        });

        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).inner_margin(Margin::ZERO))
            .show_inside(ui, |ui| {
                Table::new()
                    .columns(
                        self.columns()
                            .iter()
                            .map(|col| egui_table::Column::new(col.width()).id(egui::Id::new(col)))
                            .chain(iter::once(
                                egui_table::Column::new(150.)
                                    .id("padding".into())
                                    .resizable(false),
                            ))
                            .collect::<Vec<_>>(),
                    )
                    .headers([HeaderRow { height: 24.0, groups: vec![] }])
                    .num_rows(self.global.assets.len() as u64)
                    .show(ui, self);
            });
    }
}

impl TableDelegate for AssetsList<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let Some(column) = &self.columns().get(cell.col_range.start) else {
                        return;
                    };

                    ui.label(column.header(self.global));
                });
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let Some(asset) = self.global.assets.get(cell.row_nr as usize) else {
            return;
        };

        let Some(column) = self.columns().get(cell.col_nr) else {
            return;
        };

        ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            column.frame().show(ui, |ui| {
                column.contents(ui, self.global, asset);
            });
        });
    }

    fn row_ui(&mut self, ui: &mut egui::Ui, row_nr: u64) {
        let odd_row = row_nr % 2 == 1;

        let res = ui
            .response()
            .interact(Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        if res.clicked() {
            if let Some(asset) = self.global.assets.get(row_nr as usize) {
                self.elm
                    .send(Message::ChangePage(HamPage::EditAsset(Some(asset.id))));
            }
        } else if res.hovered() {
            if res.is_pointer_button_down_on() {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.visuals().widgets.active.bg_fill);
            } else {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.visuals().widgets.hovered.bg_fill);
            }
        } else if odd_row {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }
    }
}
