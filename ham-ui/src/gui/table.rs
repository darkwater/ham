use std::sync::Arc;

use egui::{Frame, Layout, UiBuilder, mutex::RwLock};
use egui_table::{HeaderCellInfo, HeaderRow, Table, TableDelegate};
use ham_shared::assets::Asset;

use super::remote::RemoteState;

pub struct AssetTable<'a> {
    remote: &'a RemoteState,
    columns: Vec<AssetColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetColumn {
    Tag,
    Category,
    DisplayName,
    Field(i64),
}

impl AssetColumn {
    pub fn width(&self) -> f32 {
        match self {
            AssetColumn::Tag => 80.0,
            AssetColumn::Category => 100.0,
            AssetColumn::DisplayName => 200.0,
            AssetColumn::Field(_) => 150.0,
        }
    }

    fn header(&self) -> String {
        match self {
            AssetColumn::Tag => "Tag".to_string(),
            AssetColumn::Category => "Category".to_string(),
            AssetColumn::DisplayName => "Display Name".to_string(),
            AssetColumn::Field(field_id) => format!("Field {}", field_id),
        }
    }

    #[expect(clippy::unused_self)]
    pub fn frame(&self) -> egui::Frame {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 3))
    }

    pub fn contents(&self, ui: &mut egui::Ui, asset: &Asset) {
        match self {
            AssetColumn::Tag => {
                // TODO: tag formatting
                ui.label(format!("A{:03}", asset.id));
            }
            AssetColumn::Category => {
                ui.label(format!("C{:03}", asset.category_id));
            }
            AssetColumn::DisplayName => {
                ui.label(&asset.display_name);
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

impl<'a> AssetTable<'a> {
    pub fn new(remote: &'a RemoteState) -> Self {
        let mut columns = vec![
            AssetColumn::Tag,
            AssetColumn::Category,
            AssetColumn::DisplayName,
        ];

        for asset in remote.assets() {
            for field in &asset.fields {
                if columns
                    .iter()
                    .all(|col| *col != AssetColumn::Field(field.field_id))
                {
                    columns.push(AssetColumn::Field(field.field_id));
                }
            }
        }

        Self { remote, columns }
    }

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
            .headers([HeaderRow {
                height: 24.0,
                groups: vec![],
            }])
            .num_rows(self.remote.assets().len() as u64 + 1)
            .show(ui, self);
    }

    fn new_asset_ui(&self, remote: &RemoteState, ui: &mut egui::Ui, col_nr: usize) {
        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.max_rect().shrink(2.))
                .layout(Layout::top_down_justified(egui::Align::Center)),
        );

        match col_nr {
            0 => {
                ui.button("+");
            }
            1 => {
                let new_category_id = ui.memory_mut(|m| {
                    m.data
                        .get_temp_mut_or_default::<Arc<RwLock<Option<i64>>>>(egui::Id::new(
                            "new asset category",
                        ))
                        .clone()
                });

                let selected_text = match *new_category_id.read() {
                    Some(category_id) => match remote.category(category_id) {
                        Some(category) => {
                            format!("({}) {}", category.id, category.display_name)
                        }
                        None => format!("Unknown category ({})", category_id),
                    },
                    None => String::new(),
                };

                egui::ComboBox::from_id_salt("new asset category")
                    .selected_text(selected_text)
                    .show_ui(&mut ui, |ui| {
                        for category in remote.categories() {
                            ui.selectable_value(
                                &mut *new_category_id.write(),
                                Some(category.id),
                                format!("({}) {}", category.id, category.display_name),
                            );
                        }
                    });
            }
            2 => {
                let new_name = ui.memory_mut(|m| {
                    m.data
                        .get_temp_mut_or_default::<Arc<RwLock<String>>>(egui::Id::new(
                            "new asset name",
                        ))
                        .clone()
                });

                ui.add(egui::TextEdit::singleline(&mut *new_name.write()).hint_text("New asset"));
            }
            _ => {}
        }
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

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let Some(column) = &self.columns.get(cell.col_range.start) else {
                        return;
                    };

                    ui.label(column.header());
                });
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let Some(asset) = self.remote.assets().get(cell.row_nr as usize) else {
            self.new_asset_ui(self.remote, ui, cell.col_nr);

            return;
        };

        let Some(column) = self.columns.get(cell.col_nr) else {
            return;
        };

        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            column.frame().show(ui, |ui| {
                column.contents(ui, asset);
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

    fn default_row_height(&self) -> f32 {
        28.
    }
}
