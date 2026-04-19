mod asset;
mod assets_list;
mod categories;
mod index;

use egui::{Align, Frame, Layout, Margin, Vec2};
use egui_elm::{App, ElmCtx, Fragment, Task};
use ham_shared::{
    Asset, AssetId, Category, CategoryId, CommaSeparated, CreateAssetParams, CreateCategoryParams,
    Field, FieldId, ListAssetParams,
};
use serde::{Deserialize, Serialize};

use self::{asset::AssetPage, assets_list::AssetColumn};
use crate::gui::{assets_list::AssetsList, categories::CategoriesPage, index::Index};

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
    index: Index,

    settings: Settings,

    categories_selection: Option<CategoryId>,
    asset_edit_mode: bool,
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
    fn asset(&self, id: AssetId) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

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
    AssetCreated(surf::Result<Asset>),
    AssetUpdated(AssetId, surf::Result<Asset>),
    CategoriesLoaded(surf::Result<Vec<Category>>),
    CategoryCreated(surf::Result<Category>),
    CategoryDeleted(CategoryId, surf::Result<()>),
    FieldsLoaded(surf::Result<Vec<Field>>),

    ChangePage(HamPage),

    ToggleFetchAssetField(FieldId, bool),

    SetAssetEditMode(bool),
    CreateAsset(CreateAssetParams),
    UpdateAsset(AssetId, CreateAssetParams),

    SelectCategory(CategoryId),
    CreateCategory(CreateCategoryParams),
    DeleteCategory(CategoryId),
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
    const MENU: &[HamPage] =
        &[HamPage::Assets, HamPage::Categories, HamPage::Fields, HamPage::EditAsset(None)];

    fn title(&self) -> &str {
        match self {
            HamPage::Assets => "Assets",
            HamPage::Categories => "Categories",
            HamPage::Fields => "Fields",
            HamPage::EditAsset(None) => "New Asset",
            HamPage::EditAsset(Some(_)) => "Edit Asset",
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
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }
            Message::AssetCreated(result) => {
                let asset = self.handle_surf_err(result);
                if let HamPage::EditAsset(None) = self.page {
                    self.page = HamPage::EditAsset(Some(asset.id));
                }
                self.global.asset_edit_mode = false;
                self.global.assets.push(asset);
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }
            Message::AssetUpdated(asset_id, result) => {
                let updated_asset = self.handle_surf_err(result);
                if let Some(asset) = self.global.assets.iter_mut().find(|a| a.id == asset_id) {
                    *asset = updated_asset;
                }
                self.global.asset_edit_mode = false;
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }
            Message::CategoriesLoaded(result) => {
                self.global.categories = self.handle_surf_err(result);
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }
            Message::FieldsLoaded(result) => {
                self.global.fields = self.handle_surf_err(result);
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }
            Message::CategoryCreated(result) => {
                let category = self.handle_surf_err(result);
                self.global.categories.push(category);
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }
            Message::CategoryDeleted(category_id, result) => {
                let () = self.handle_surf_err(result);
                self.global.categories.retain(|c| c.id != category_id);
                self.global.index = Index::calculate(&self.global);
                Task::none()
            }

            Message::ChangePage(page) => {
                self.page = page;
                self.global.asset_edit_mode = matches!(page, HamPage::EditAsset(None));
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

            Message::SetAssetEditMode(edit_mode) => {
                self.global.asset_edit_mode = edit_mode;
                Task::none()
            }
            Message::CreateAsset(params) => self.create_asset(params),
            Message::UpdateAsset(asset_id, params) => self.update_asset(asset_id, params),

            Message::SelectCategory(category_id) => {
                self.global.categories_selection = Some(category_id);
                Task::none()
            }

            Message::CreateCategory(params) => self.create_category(params),
            Message::DeleteCategory(category_id) => self.delete_category(category_id),
        }
    }

    fn view(&self, ui: &mut egui::Ui, _frame: &mut eframe::Frame, elm: ElmCtx<Self::Message>) {
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
                });
            });

        match self.page {
            HamPage::Assets => AssetsList { global: &self.global, elm }.show(ui),
            HamPage::Categories => CategoriesPage { global: &self.global, elm }.show(ui),
            HamPage::Fields => todo!(),
            HamPage::EditAsset(asset_id) => {
                AssetPage { global: &self.global, elm, asset_id }.show(ui)
            }
        }
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

    fn post(&self, path: &str) -> surf::RequestBuilder {
        surf::post(self.url(path))
    }

    fn patch(&self, path: &str) -> surf::RequestBuilder {
        surf::patch(self.url(path))
    }

    fn handle_surf_err<T>(&self, result: surf::Result<T>) -> T {
        match result {
            Ok(value) => value,
            Err(e) => {
                todo!("Handle surf error: {e}");
            }
        }
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

    fn create_asset(&self, params: CreateAssetParams) -> Task<Message> {
        Task::perform(
            self.post("assets")
                .body_json(&params)
                .unwrap()
                .recv_json::<Asset>(),
            Message::AssetCreated,
        )
    }

    fn update_asset(&self, asset_id: AssetId, params: CreateAssetParams) -> Task<Message> {
        Task::perform(
            self.patch(&format!("assets/{}", asset_id.0))
                .body_json(&params)
                .unwrap()
                .recv_json::<Asset>(),
            move |res| Message::AssetUpdated(asset_id, res),
        )
    }

    fn load_categories(&self) -> Task<Message> {
        Task::perform(
            self.get("categories").recv_json::<Vec<Category>>(),
            Message::CategoriesLoaded,
        )
    }

    fn create_category(&self, params: CreateCategoryParams) -> Task<Message> {
        Task::perform(
            self.post("categories")
                .body_json(&params)
                .unwrap()
                .recv_json::<Category>(),
            Message::CategoryCreated,
        )
    }

    fn delete_category(&self, category_id: CategoryId) -> Task<Message> {
        Task::perform(
            surf::delete(self.url(&format!("categories/{}", category_id.0))).recv_bytes(),
            move |res| Message::CategoryDeleted(category_id, res.map(|_empty| ())),
        )
    }

    fn load_fields(&self) -> Task<Message> {
        Task::perform(self.get("fields").recv_json::<Vec<Field>>(), Message::FieldsLoaded)
    }
}
