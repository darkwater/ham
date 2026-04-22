use egui::Key;
use egui_elm::{EguiUiExt as _, ElmCtx};
use ham_shared::{Category, CreateCategoryParams};

use crate::gui::{GlobalState, Message};

pub struct CategoriesPage<'a> {
    pub global: &'a GlobalState,
    pub elm: ElmCtx<'a, Message>,
}

impl<'a> CategoriesPage<'a> {
    pub fn show(self, ui: &mut egui::Ui) {
        if let Some(category_id) = self.global.categories_selection {
            egui::Panel::right("details")
                .default_size(ui.available_width() / 2.)
                .frame(egui::Frame::central_panel(ui.style()))
                .show_inside(ui, |ui| {
                    ui.heading(
                        CategoryAncestryIter {
                            global: self.global,
                            current: self.global.category(category_id),
                        }
                        .display(),
                    );

                    ui.add_space(12.);
                    ui.heading("Create subcategory");

                    let name = ui.hold_value("new category name", "");
                    {
                        let mut name = name.lock();
                        let res = ui.text_edit_singleline(&mut *name);
                        if res.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.elm.send(Message::CreateCategory(CreateCategoryParams {
                                display_name: name.clone(),
                                parent_id: Some(category_id),
                                field_ids: vec![],
                            }));

                            *name = String::new();
                        }
                    }

                    ui.add_space(12.);
                    ui.heading("Delete category");

                    let num_assets = self
                        .global
                        .index
                        .category_asset_count(category_id)
                        .unwrap_or(0);

                    if num_assets > 0 {
                        ui.label(format!(
                            "This category has {} assets. \
                            You must move or delete them before you can delete the category.",
                            num_assets
                        ));

                        ui.label(
                            "You can't delete a category with subcategories either, \
                            but I don't check for that yet.",
                        );
                    } else {
                        ui.menu_button("Delete", |ui| {
                            if ui.button("Confirm").clicked() {
                                self.elm.send(Message::DeleteCategory(category_id));
                            }
                        });
                    }
                });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Categories");

            for category in &self.global.categories {
                let mut ancestry = CategoryAncestryIter {
                    global: self.global,
                    current: Some(category),
                }
                .collect::<Vec<_>>();
                ancestry.reverse();

                ui.horizontal(|ui| {
                    for (i, cat) in ancestry.iter().enumerate() {
                        if i > 0 {
                            ui.label("/");
                        }

                        let selected = self.global.categories_selection == Some(cat.id);

                        let res = ui.selectable_label(selected, &cat.display_name);

                        if res.clicked() {
                            self.elm.send(Message::SelectCategory(cat.id));
                        }
                    }

                    ui.add_space(4.);

                    ui.weak(format!(
                        "({} assets)",
                        self.global
                            .index
                            .category_asset_count(category.id)
                            .unwrap_or(0)
                    ));
                });
            }
        });
    }
}

pub struct CategoryAncestryIter<'a> {
    pub global: &'a GlobalState,
    pub current: Option<&'a Category>,
}

impl<'a> Iterator for CategoryAncestryIter<'a> {
    type Item = &'a Category;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            self.current = current.parent_id.and_then(|pid| self.global.category(pid));

            Some(current)
        } else {
            None
        }
    }
}

impl<'a> CategoryAncestryIter<'a> {
    pub fn into_vec(self) -> Vec<&'a Category> {
        let mut ancestry = self.collect::<Vec<_>>();
        ancestry.reverse();
        ancestry
    }

    pub fn display(self) -> String {
        self.into_vec()
            .iter()
            .map(|c| c.display_name.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }
}
