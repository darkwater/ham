use std::collections::BTreeSet;

use ham_shared::{Asset, Category, CommaSeparated, FieldId, ListAssetParams};
use iced::{Font, Task, font};

pub fn main() -> iced::Result {
    iced::application(Ham::new, Ham::update, Ham::view)
        .theme(Ham::theme)
        .subscription(Ham::subscription)
        .title(Ham::title)
        .run()
}

struct Ham {
    assets: Vec<Asset>,
    categories: Vec<Category>,
    page: HamPage,
}

enum HamPage {
    Loading,
    Assets,
}

enum Message {
    AssetsLoaded(surf::Result<Vec<Asset>>),
    CategoriesLoaded(surf::Result<Vec<Category>>),
}

impl Ham {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                assets: Vec::new(),
                categories: Vec::new(),
                page: HamPage::Loading,
            },
            Task::batch([Self::load_categories(), Self::load_assets()]),
        )
    }

    fn theme(&self) -> iced::theme::Theme {
        iced::theme::Theme::Dark
    }

    fn title(&self) -> String {
        "Ham".to_string()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::AssetsLoaded(Ok(assets)) => {
                self.assets = assets;
                self.page = HamPage::Assets
            }
            Message::CategoriesLoaded(Ok(categories)) => {
                self.categories = categories;
            }
            Message::AssetsLoaded(Err(e)) => {
                todo!("Failed to load assets: {e:?}")
            }
            Message::CategoriesLoaded(Err(e)) => {
                todo!("Failed to load categories: {e:?}")
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        use iced::widget::*;

        match &self.page {
            HamPage::Loading => text("Loading").size(50).into(),
            HamPage::Assets => {
                let fields = self
                    .assets
                    .iter()
                    .flat_map(|asset| asset.fields.iter().map(|field| field.field_id))
                    .collect::<BTreeSet<_>>();

                let header = |label| {
                    text(label).font(Font {
                        weight: font::Weight::Bold,
                        ..Font::DEFAULT
                    })
                };

                let mut columns = vec![
                    table::column(header("Tag".to_owned()), |asset: &Asset| {
                        text(asset.id.0.to_string())
                    }),
                    table::column(header("Category".to_owned()), |asset: &Asset| {
                        if let Some(category) =
                            self.categories.iter().find(|c| c.id == asset.category_id)
                        {
                            text(&category.display_name)
                        } else {
                            text("-")
                        }
                    }),
                    table::column(header("Name".to_owned()), |asset: &Asset| {
                        text(&asset.display_name)
                    }),
                ];

                for field_id in fields {
                    columns.push(table::column(
                        header(format!("Field {}", field_id.0)),
                        move |asset: &Asset| {
                            asset
                                .fields
                                .iter()
                                .find(|field| field.field_id == field_id)
                                .map(|field| text(field.value.to_string()))
                                .unwrap_or_else(|| text("-"))
                        },
                    ));
                }

                table(columns, &self.assets).into()
            }
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
}

impl Ham {
    fn load_assets() -> Task<Message> {
        Task::perform(
            surf::get("http://localhost:6172/assets")
                .query(&ListAssetParams {
                    field_ids: CommaSeparated::from_slice(&[FieldId(1)]),
                })
                .unwrap()
                .recv_json::<Vec<Asset>>(),
            Message::AssetsLoaded,
        )
    }

    fn load_categories() -> Task<Message> {
        Task::perform(
            surf::get("http://localhost:6172/categories").recv_json::<Vec<Category>>(),
            Message::CategoriesLoaded,
        )
    }
}
