mod assets;
mod categories;

use std::sync::Arc;

use ham_shared::{
    Asset, AssetId, Category, CategoryId, CommaSeparated, Field, FieldId, ListAssetParams,
};
use iced::{
    Padding, Task,
    futures::FutureExt as _,
    widget::{row, rule},
};
use iced_aw::{sidebar::TabLabel, style::Status, widget::Sidebar};

pub fn main() -> iced::Result {
    iced::application(Ham::new, Ham::update, Ham::view)
        .theme(Ham::theme)
        .subscription(Ham::subscription)
        .title(Ham::title)
        .run()
}

#[derive(Default)]
struct Ham {
    global: GlobalState,
    page: HamPage,
    assets_state: assets::State,
    categories_state: categories::State,
}

#[derive(Debug, Default)]
struct GlobalState {
    assets: Vec<Asset>,
    categories: Vec<Category>,
    fields: Vec<Field>,

    fetch_asset_fields: Vec<FieldId>,
}

impl GlobalState {
    fn category(&self, category_id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == category_id)
    }

    fn field(&self, field_id: FieldId) -> Option<&Field> {
        self.fields.iter().find(|f| f.id == field_id)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum HamPage {
    #[default]
    Assets,
    Categories,
    Fields,
    EditAsset(AssetId),
}

#[derive(Debug, Clone)]
enum Message {
    AssetsLoaded(Arc<surf::Result<Vec<Asset>>>),
    CategoriesLoaded(Arc<surf::Result<Vec<Category>>>),
    FieldsLoaded(Arc<surf::Result<Vec<Field>>>),

    ChangePage(HamPage),
    CategoriesPage(categories::CategoriesMessage),

    RefreshAssets,
    ToggleFetchAssetField(FieldId, bool),
}

impl Ham {
    fn new() -> (Self, Task<Message>) {
        let this = Self::default();
        let init = this.init();

        (this, init)
    }

    fn init(&self) -> Task<Message> {
        Task::batch([Self::load_categories(), self.load_assets(), Self::load_fields()])
    }

    fn theme(&self) -> iced::theme::Theme {
        iced::theme::Theme::TokyoNight
    }

    fn title(&self) -> String {
        "Ham".to_string()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AssetsLoaded(assets) => {
                self.global.assets = self.handle_surf_err_arc(assets);
            }
            Message::CategoriesLoaded(categories) => {
                self.global.categories = self.handle_surf_err_arc(categories);
            }
            Message::FieldsLoaded(fields) => {
                self.global.fields = self.handle_surf_err_arc(fields);
            }

            Message::ChangePage(page) => {
                self.page = page;
            }

            Message::CategoriesPage(categories_message) => {
                return self.categories_state.update(categories_message);
            }

            Message::RefreshAssets => return self.load_assets(),

            Message::ToggleFetchAssetField(field_id, enabled) => {
                if enabled {
                    self.global.fetch_asset_fields.push(field_id);
                } else {
                    self.global.fetch_asset_fields.retain(|&id| id != field_id);
                }
            }
        }

        Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let page = match &self.page {
            HamPage::Assets => self.assets_state.view(&self.global),
            HamPage::Categories => self.categories_state.view(&self.global),
            HamPage::Fields => self.assets_state.view(&self.global),
            HamPage::EditAsset(id) => self.assets_state.view(&self.global),
        };

        let sidebar = Sidebar::new(Message::ChangePage)
            .push(HamPage::Assets, TabLabel::Text("Assets".to_string()))
            .push(HamPage::Categories, TabLabel::Text("Categories".to_string()))
            .push(HamPage::Fields, TabLabel::Text("Fields".to_string()))
            .set_active_tab(&self.page)
            .padding(Padding::ZERO.right(10.))
            .style(|theme, status| {
                use iced_aw::style::sidebar::Catalog;
                let catalog = <iced::theme::Theme as Catalog>::default();
                let mut style = catalog(theme, status);

                style.tab_label_border_width = 0.;
                if matches!(status, Status::Active | Status::Hovered) {
                    style.text_color = theme.extended_palette().primary.base.text;
                }

                style
            });

        row![sidebar, rule::vertical(1.), page].into()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
}

impl Ham {
    fn load_assets(&self) -> Task<Message> {
        Task::perform(
            surf::get("http://localhost:6172/assets")
                .query(&ListAssetParams {
                    field_ids: CommaSeparated::from_slice(&self.global.fetch_asset_fields),
                })
                .unwrap()
                .recv_json::<Vec<Asset>>()
                .map(Arc::new),
            Message::AssetsLoaded,
        )
    }

    fn load_categories() -> Task<Message> {
        Task::perform(
            surf::get("http://localhost:6172/categories")
                .recv_json::<Vec<Category>>()
                .map(Arc::new),
            Message::CategoriesLoaded,
        )
    }

    fn load_fields() -> Task<Message> {
        Task::perform(
            surf::get("http://localhost:6172/fields")
                .recv_json::<Vec<Field>>()
                .map(Arc::new),
            Message::FieldsLoaded,
        )
    }

    fn handle_surf_err_arc<T>(&self, result: Arc<surf::Result<T>>) -> T {
        let Ok(result) = Arc::try_unwrap(result) else {
            panic!("Arc<surf::Result<T>> had multiple strong refs");
        };

        match result {
            Ok(value) => value,
            Err(e) => {
                todo!("Handle surf error: {e}");
            }
        }
    }
}
