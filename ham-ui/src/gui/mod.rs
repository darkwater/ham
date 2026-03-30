use std::sync::Arc;

use egui::{CentralPanel, Frame, SidePanel, mutex::RwLock};
use ham_shared::categories::{Category, CreateCategoryParams};

use self::{remote::RemoteState, table::AssetTable};

mod remote;
mod table;

pub fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ham",
        native_options,
        Box::new(|cc| Ok(Box::new(HamApp::new(cc)))),
    )
}

struct HamApp {
    remote: RemoteState,
    current_page: Page,
}

impl HamApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            remote: RemoteState::new(),
            current_page: Page::default(),
        }
    }
}

impl eframe::App for HamApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.remote.poll_refresh();

        SidePanel::left("menu").show(ctx, |ui| {
            for page in Page::ALL {
                ui.selectable_value(&mut self.current_page, *page, page.title());
            }
        });

        { self.current_page }.show(self, ctx);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum Page {
    #[default]
    Assets,
    Categories,
}

impl Page {
    const ALL: &[Page] = &[Page::Assets, Page::Categories];

    fn title(&self) -> &str {
        match self {
            Page::Assets => "Assets",
            Page::Categories => "Categories",
        }
    }

    fn frame(&self, ctx: &egui::Context) -> Frame {
        match self {
            Page::Assets => Frame::central_panel(&ctx.style()).inner_margin(0),
            Page::Categories => Frame::central_panel(&ctx.style()),
        }
    }

    fn contents(&self, app: &mut HamApp, ui: &mut egui::Ui) {
        match self {
            Page::Assets => {
                AssetTable::new(&app.remote).show(ui);
            }
            Page::Categories => {
                fn cat_ui(cat: Category, ui: &mut egui::Ui, categories: &[Category]) {
                    let children = categories
                        .iter()
                        .filter(|c| c.parent_id == Some(cat.id))
                        .peekable();

                    let label = format!("({}) {}", cat.id, cat.display_name);

                    // if children.peek().is_some() {
                    ui.collapsing(label, |ui| {
                        for child in children {
                            cat_ui(child.clone(), ui, categories);
                        }

                        let new_name = ui.memory_mut(|m| {
                            m.data
                                .get_temp_mut_or_default::<Arc<RwLock<String>>>(
                                    egui::Id::new("new cat name").with(cat.id),
                                )
                                .clone()
                        });

                        ui.text_edit_singleline(&mut *new_name.write())
                            .on_hover_text("New category name");

                        if ui.button("Add subcategory").clicked() {
                            tokio::spawn(async move {
                                let category = CreateCategoryParams {
                                    display_name: new_name.read().clone(),
                                    parent_id: Some(cat.id),
                                };

                                surf::post("http://localhost:6172/categories")
                                    .body_json(&category)
                                    .unwrap()
                                    .await
                                    .unwrap();

                                remote::QUEUED
                                    .write()
                                    .unwrap()
                                    .replace(remote::QueueRefresh::Categories);
                            });
                        }
                    });
                    // } else {
                    //     ui.label(label);
                    // }
                }

                if let Some(categories) = app.remote.categories.ready() {
                    for cat in categories.iter().filter(|c| c.parent_id.is_none()) {
                        cat_ui(cat.clone(), ui, categories);
                    }
                }
            }
        }
    }

    fn show(&self, app: &mut HamApp, ctx: &egui::Context) {
        CentralPanel::default()
            .frame(self.frame(ctx))
            .show(ctx, |ui| {
                self.contents(app, ui);
            });
    }
}
