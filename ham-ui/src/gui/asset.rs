use core::mem;

use egui::{Frame, Margin, TextEdit};
use egui_elm::{EguiUiExt as _, ElmCtx};
use ham_shared::{AssetId, CreateAssetParams};

use super::{GlobalState, Message, categories::CategoryAncestryIter};

pub struct AssetPage<'a> {
    pub global: &'a GlobalState,
    pub elm: ElmCtx<'a, Message>,
    pub asset_id: Option<AssetId>,
}

#[derive(Debug, Clone, Copy, Hash)]
enum HoldId {
    Name,
    Category,
}

impl HoldId {
    fn id(self, asset_id: Option<AssetId>) -> egui::Id {
        egui::Id::new((self, asset_id))
    }
}

impl AssetPage<'_> {
    pub fn show(&self, ui: &mut egui::Ui) {
        let asset = self.asset_id.and_then(|id| self.global.asset(id));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::Grid::new("asset grid")
                .num_columns(2)
                // .striped(true)
                // .min_col_width(ui.available_width() / 4.)
                .spacing([24., 8.])
                .show(ui, |ui| {
                    ui.label("");
                    ui.add_enabled_ui(self.asset_id.is_some(), |ui| {
                        let mut edit = self.global.asset_edit_mode;
                        let res = ui.checkbox(&mut edit, "Edit mode");
                        if res.changed() {
                            self.elm.send(Message::SetAssetEditMode(edit));
                        }
                    });
                    ui.end_row();

                    ui.label("ID");
                    ui.label(asset.map_or_else(|| "New asset".to_string(), |a| a.id.to_string()));
                    ui.end_row();

                    ui.label("Name");
                    if self.global.asset_edit_mode {
                        let initial_value =
                            asset.map(|a| a.display_name.as_str()).unwrap_or_default();

                        let name = ui.hold_value(HoldId::Name.id(self.asset_id), initial_value);
                        let mut name = name.lock();

                        Frame::new()
                            .inner_margin(Margin::symmetric(-4, -2))
                            .show(ui, |ui| {
                                TextEdit::singleline(&mut *name).show(ui);
                            });
                    } else {
                        ui.label(asset.map(|a| a.display_name.as_str()).unwrap_or_default());
                    }
                    ui.end_row();

                    ui.label("Category");
                    let mut category_set = None;
                    if self.global.asset_edit_mode {
                        let initial_value = asset.map(|a| a.category_id);

                        let category_id =
                            ui.hold_value(HoldId::Category.id(self.asset_id), &initial_value);

                        let mut category_id = category_id.lock();

                        Frame::new()
                            .inner_margin(Margin::symmetric(-4, -2))
                            .show(ui, |ui| {
                                egui::ComboBox::from_id_salt(("category combo", self.asset_id))
                                    .selected_text(
                                        category_id
                                            .and_then(|id| self.global.category(id))
                                            .map(|c| c.display_name.as_str())
                                            .unwrap_or("<none>"),
                                    )
                                    .show_ui(ui, |ui| {
                                        for category in &self.global.categories {
                                            let label = CategoryAncestryIter {
                                                global: self.global,
                                                current: Some(category),
                                            }
                                            .display();

                                            ui.selectable_value(
                                                &mut *category_id,
                                                Some(category.id),
                                                label,
                                            );
                                        }
                                    });
                            });

                        category_set = *category_id;
                    } else {
                        ui.label(
                            asset
                                .and_then(|a| self.global.category(a.category_id))
                                .map(|c| c.display_name.clone())
                                .unwrap_or("<unknown>".to_string()),
                        );
                    }
                    ui.end_row();

                    ui.label("");
                    if self.global.asset_edit_mode {
                        ui.add_enabled_ui(category_set.is_some(), |ui| {
                            if ui.button("Save").clicked() {
                                let display_name = mem::take(
                                    &mut *ui.hold_value(HoldId::Name.id(self.asset_id), "").lock(),
                                );

                                let category_id = category_set.unwrap();

                                let params = CreateAssetParams { category_id, display_name };

                                if let Some(asset_id) = self.asset_id {
                                    self.elm.send(Message::UpdateAsset(asset_id, params));
                                } else {
                                    self.elm.send(Message::CreateAsset(params));
                                }
                            }
                        });
                    }

                    // // abuse empty row to expand the grid to fill the parent
                    // ui.label("");
                    // ui.allocate_at_least(ui.available_size(), Sense::hover());
                    // ui.end_row();
                });
        });
    }
}
