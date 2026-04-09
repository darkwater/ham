use std::sync::Arc;

use egui::{Align, CentralPanel, Frame, Layout, Margin, Panel, TextEdit, Vec2, mutex::RwLock};

use self::table::{AssetColumn, AssetTable};
use crate::db::{AssetDb, AssetId, Category, Field, FieldType};

mod table;

pub fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native("Ham", native_options, Box::new(|cc| Ok(Box::new(HamApp::new(cc)))))
}

struct HamApp {
    db: AssetDb,
    db_loaded: bool,
    current_page: Page,
    asset_table_columns: Vec<AssetColumn>,
}

impl HamApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let db = cc.storage.and_then(|s| eframe::get_value(s, "asset db"));

        cc.egui_ctx.all_styles_mut(|s| {
            s.interaction.selectable_labels = false;
        });

        Self {
            db_loaded: db.is_some(),
            db: db.unwrap_or_default(),
            current_page: Page::default(),
            asset_table_columns: AssetColumn::BASE.to_vec(),
        }
    }
}

impl eframe::App for HamApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if !self.db_loaded {
            ui.heading("Warning!");
            ui.label("Asset database failed to load. Create a new one?");
            if ui.button("Create new").clicked() {
                self.db_loaded = true;
                self.db.categories.push(Category {
                    id: self.db.next_category_id(),
                    display_name: "Root".to_string(),
                    parent_id: None,
                });
            }
            return;
        }

        Panel::left("menu")
            .frame(
                Frame::side_top_panel(ui.style()).inner_margin(Margin { right: 1, ..Margin::ZERO }),
            )
            .show_inside(ui, |ui| {
                ui.style_mut().spacing.item_spacing = Vec2::ZERO;
                ui.style_mut().spacing.button_padding = egui::vec2(12., 6.);

                ui.visuals_mut().widgets.open.corner_radius = 0.0.into();
                ui.visuals_mut().widgets.active.corner_radius = 0.0.into();
                ui.visuals_mut().widgets.hovered.corner_radius = 0.0.into();
                ui.visuals_mut().widgets.inactive.corner_radius = 0.0.into();
                ui.visuals_mut().widgets.noninteractive.corner_radius = 0.0.into();

                ui.visuals_mut().widgets.hovered.bg_stroke.width = 0.;

                ui.with_layout(Layout::top_down(Align::LEFT).with_cross_justify(true), |ui| {
                    for page in Page::ALL {
                        ui.selectable_value(&mut self.current_page, *page, page.title());
                    }

                    ui.separator();

                    if ui.button("New asset").clicked() {
                        let id = self.db.create_asset();
                        self.current_page = Page::EditAsset(id);
                    }
                });
            });

        { self.current_page }.show(self, ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.db_loaded {
            eframe::set_value(storage, "asset db", &self.db);
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum Page {
    #[default]
    Assets,
    Categories,
    Fields,
    EditAsset(AssetId),
}

impl Page {
    const ALL: &[Page] = &[Page::Assets, Page::Categories, Page::Fields];

    fn title(&self) -> &str {
        match self {
            Page::Assets => "Assets",
            Page::Categories => "Categories",
            Page::Fields => "Fields",
            Page::EditAsset(_) => "Edit Asset",
        }
    }

    fn frame(&self, ui: &egui::Ui) -> Frame {
        match self {
            Page::Assets => Frame::central_panel(ui.style()).inner_margin(0),
            _ => Frame::central_panel(ui.style()),
        }
    }

    fn contents(&self, app: &mut HamApp, ui: &mut egui::Ui) {
        match self {
            Page::Assets => {
                AssetTable {
                    db: &app.db,
                    columns: &mut app.asset_table_columns,
                }
                .show(ui);
            }
            Page::Categories => {
                let selected_id = egui::Id::new("selected category");
                let selected_cat = ui.memory_mut(|m| m.data.get_temp::<Category>(selected_id));

                for cat in &app.db.categories {
                    let ancestry = CategoryAncestryIter { db: &app.db, current: Some(cat) }
                        .collect::<Vec<_>>();

                    ui.horizontal(|ui| {
                        for (idx, category) in ancestry.into_iter().rev().enumerate() {
                            if idx > 0 {
                                ui.label(">");
                            }

                            let res = ui.selectable_label(
                                selected_cat.as_ref().map(|c| c.id) == Some(category.id),
                                &category.display_name,
                            );

                            if res.clicked() {
                                ui.memory_mut(|m| {
                                    m.data.insert_temp(selected_id, category.clone())
                                });
                            }
                        }
                    });
                }

                if let Some(cat) = selected_cat {
                    ui.add_space(16.);

                    ui.heading("Create category");

                    ui.horizontal(|ui| {
                        let ancestry = CategoryAncestryIter { db: &app.db, current: Some(&cat) }
                            .collect::<Vec<_>>();

                        for category in ancestry.into_iter().rev() {
                            ui.label(&category.display_name);
                            ui.label(">");
                        }

                        let new_name = ui.memory_mut(|m| {
                            m.data
                                .get_temp_mut_or_default::<Arc<RwLock<String>>>(
                                    egui::Id::new("new cat name").with(cat.id),
                                )
                                .clone()
                        });

                        TextEdit::singleline(&mut *new_name.write())
                            .hint_text("New category name")
                            .desired_width(150.)
                            .show(ui);

                        if ui.button("Create").clicked() {
                            let new_cat = Category {
                                id: app.db.next_category_id(),
                                display_name: new_name.read().clone(),
                                parent_id: Some(cat.id),
                            };

                            app.db.categories.push(new_cat.clone());
                        }
                    });
                }
            }
            Page::Fields => {
                egui::Grid::new("fields grid")
                    .num_columns(2)
                    .spacing(Vec2::splat(8.))
                    .min_col_width(150.)
                    .show(ui, |ui| {
                        for field in &mut app.db.fields {
                            ui.text_edit_singleline(&mut field.display_name);
                            ui.add(&mut field.field_type);
                            ui.end_row();
                        }

                        let new_name = ui.memory_mut(|m| {
                            m.data
                                .get_temp_mut_or_default::<Arc<RwLock<String>>>(egui::Id::new(
                                    "new field name",
                                ))
                                .clone()
                        });

                        TextEdit::singleline(&mut *new_name.write())
                            .hint_text("New field name")
                            .desired_width(200.)
                            .show(ui);

                        if ui.button("Create").clicked() {
                            let new_field = Field {
                                id: app.db.next_field_id(),
                                display_name: new_name.read().clone(),
                                field_type: FieldType::default(),
                            };

                            app.db.fields.push(new_field);

                            new_name.write().clear();
                        }
                    });
            }
            Page::EditAsset(id) => {
                let Some(asset) = app.db.asset_mut(*id) else {
                    ui.label("Asset not found");
                    return;
                };
            }
        }
    }

    fn show(&self, app: &mut HamApp, ui: &mut egui::Ui) {
        CentralPanel::default()
            .frame(self.frame(ui))
            .show_inside(ui, |ui| {
                self.contents(app, ui);
            });
    }
}

struct CategoryAncestryIter<'a> {
    db: &'a AssetDb,
    current: Option<&'a Category>,
}

impl<'a> Iterator for CategoryAncestryIter<'a> {
    type Item = &'a Category;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            self.current = current
                .parent_id
                .and_then(|pid| self.db.categories.iter().find(|c| c.id == pid));

            Some(current)
        } else {
            None
        }
    }
}
