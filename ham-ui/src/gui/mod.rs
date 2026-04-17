mod assets;

use egui::{Align, Frame, Layout, Margin, Vec2};
use egui_elm::{App, ElmCtx, Fragment, Task};
use ham_shared::{
    Asset, AssetId, Category, CategoryId, CommaSeparated, Field, FieldId, ListAssetParams,
};
use serde::{Deserialize, Serialize};

use self::assets::AssetColumn;
use crate::gui::assets::AssetTable;

pub fn main() -> eframe::Result<()> {
    egui_elm::run_app::<HamApp>("ham", Default::default())
}

#[derive(Debug, Default)]
struct HamApp {
    global: GlobalState,
    page: HamPage,
}

#[derive(Debug, Default)]
struct GlobalState {
    assets: Vec<Asset>,
    categories: Vec<Category>,
    fields: Vec<Field>,

    settings: Settings,
}

#[derive(Debug, Serialize, Deserialize)]
struct Settings {
    asset_columns: Vec<AssetColumn>,
}

impl Settings {
    const KEY: &'static str = "app settings";
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            asset_columns: AssetColumn::BASE.to_vec(),
        }
    }
}

impl GlobalState {
    fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|cat| cat.id == id)
    }

    fn field(&self, id: FieldId) -> Option<&Field> {
        self.fields.iter().find(|field| field.id == id)
    }

    fn format_asset_tag(&self, asset_id: AssetId) -> String {
        format!("A{:04}", asset_id.0)
    }
}

#[derive(Debug)]
enum Message {
    AssetsLoaded(surf::Result<Vec<Asset>>),
    CategoriesLoaded(surf::Result<Vec<Category>>),
    FieldsLoaded(surf::Result<Vec<Field>>),

    ChangePage(HamPage),

    ToggleFetchAssetField(FieldId, bool),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum HamPage {
    #[default]
    Assets,
    Categories,
    Fields,

    EditAsset(Option<AssetId>),
}

impl HamPage {
    const MENU: &[HamPage] = &[HamPage::Assets, HamPage::Categories, HamPage::Fields];

    fn title(&self) -> &str {
        match self {
            HamPage::Assets => "Assets",
            HamPage::Categories => "Categories",
            HamPage::Fields => "Fields",
            HamPage::EditAsset(_) => "Edit Asset",
        }
    }
}

impl Fragment for HamApp {
    type Message = Message;

    fn init(cc: &eframe::CreationContext) -> (Self, egui_elm::Task<Self::Message>)
    where
        Self: Sized,
    {
        cc.egui_ctx
            .all_styles_mut(|s| s.interaction.selectable_labels = false);

        let this = Self {
            global: GlobalState {
                settings: cc
                    .storage
                    .and_then(|storage| eframe::get_value(storage, Settings::KEY))
                    .unwrap_or_default(),

                ..Default::default()
            },
            ..Default::default()
        };
        let tasks =
            Task::multiple([this.load_assets(), this.load_categories(), this.load_fields()]);

        (this, tasks)
    }

    fn update(
        &mut self,
        message: Self::Message,
        _ctx: &egui::Context,
    ) -> egui_elm::Task<Self::Message> {
        match message {
            Message::AssetsLoaded(result) => {
                self.global.assets = self.handle_surf_err(result);
                Task::none()
            }
            Message::CategoriesLoaded(result) => {
                self.global.categories = self.handle_surf_err(result);
                Task::none()
            }
            Message::FieldsLoaded(result) => {
                self.global.fields = self.handle_surf_err(result);
                Task::none()
            }

            Message::ChangePage(page) => {
                self.page = page;
                Task::none()
            }

            Message::ToggleFetchAssetField(field_id, checked) => {
                if checked {
                    if !self
                        .global
                        .settings
                        .asset_columns
                        .contains(&AssetColumn::Field(field_id))
                    {
                        self.global
                            .settings
                            .asset_columns
                            .push(AssetColumn::Field(field_id));
                    }
                } else {
                    self.global
                        .settings
                        .asset_columns
                        .retain(|col| *col != AssetColumn::Field(field_id));
                }

                self.global.settings.asset_columns.sort_unstable();

                self.load_assets()
            }
        }
    }

    fn view(&self, ui: &mut egui::Ui, _frame: &mut eframe::Frame, mut elm: ElmCtx<Self::Message>) {
        egui::Panel::left("menu")
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
                    let mut current_page = self.page;
                    for page in HamPage::MENU {
                        let res = ui.selectable_value(&mut current_page, *page, page.title());
                        if res.changed() {
                            elm.send(Message::ChangePage(current_page));
                        }
                    }

                    ui.separator();
                });
            });

        egui::Panel::right("right_panel").show_inside(ui, |ui| {
            for field in &self.global.fields {
                let mut checked = self
                    .global
                    .settings
                    .asset_columns
                    .contains(&AssetColumn::Field(field.id));

                let res = ui.checkbox(&mut checked, &field.display_name);
                if res.changed() {
                    elm.send(Message::ToggleFetchAssetField(field.id, checked));
                }
            }
        });

        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).inner_margin(Margin::ZERO))
            .show_inside(ui, |ui| match self.page {
                HamPage::Assets => AssetTable { global: &self.global, elm: &mut elm }.show(ui),
                HamPage::Categories => todo!(),
                HamPage::Fields => todo!(),
                HamPage::EditAsset(asset_id) => todo!(),
            });
    }
}

impl App for HamApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, Settings::KEY, &self.global.settings);
    }
}

impl HamApp {
    fn url(&self, path: &str) -> String {
        format!("http://localhost:6172/{path}")
    }

    fn get(&self, path: &str) -> surf::RequestBuilder {
        surf::get(self.url(path))
    }

    fn load_assets(&self) -> Task<Message> {
        Task::perform(
            self.get("assets")
                .query(&ListAssetParams {
                    field_ids: CommaSeparated::from_slice(
                        &self
                            .global
                            .settings
                            .asset_columns
                            .iter()
                            .filter_map(|col| match col {
                                AssetColumn::Field(field_id) => Some(*field_id),
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    ),
                })
                .unwrap()
                .recv_json::<Vec<Asset>>(),
            Message::AssetsLoaded,
        )
    }

    fn load_categories(&self) -> Task<Message> {
        Task::perform(
            self.get("categories").recv_json::<Vec<Category>>(),
            Message::CategoriesLoaded,
        )
    }

    fn load_fields(&self) -> Task<Message> {
        Task::perform(self.get("fields").recv_json::<Vec<Field>>(), Message::FieldsLoaded)
    }

    fn handle_surf_err<T>(&self, result: surf::Result<T>) -> T {
        match result {
            Ok(value) => value,
            Err(e) => {
                todo!("Handle surf error: {e}");
            }
        }
    }
}
