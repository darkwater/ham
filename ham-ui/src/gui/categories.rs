use egui_elm::ElmCtx;
use ham_shared::Category;

use crate::gui::{GlobalState, Message};

pub struct CategoriesPage<'a> {
    pub global: &'a GlobalState,
    pub elm: &'a mut ElmCtx<Message>,
}

impl<'a> CategoriesPage<'a> {
    pub fn show(self, ui: &mut egui::Ui) {
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
                        ui.label(">");
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
    }
}

struct CategoryAncestryIter<'a> {
    global: &'a GlobalState,
    current: Option<&'a Category>,
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
