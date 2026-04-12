use std::collections::BTreeSet;

use ham_shared::Asset;
use iced::{Font, Length, Padding, font};

use crate::iced::{GlobalState, Message};

#[derive(Debug, Default)]
pub struct State {}

impl State {
    pub fn view<'a>(&'a self, global: &'a GlobalState) -> iced::Element<'a, Message> {
        use iced::widget::*;

        let table = {
            let fields = global
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
                table::column(header("Tag".to_owned()), |asset: &Asset| text(asset.id.0))
                    .width(Length::Shrink),
                table::column(header("Category".to_owned()), |asset: &Asset| {
                    if let Some(category) = global.category(asset.category_id) {
                        text(&category.display_name)
                    } else {
                        text("-")
                    }
                }),
                table::column(header("Name".to_owned()), |asset: &Asset| text(&asset.display_name)),
            ];

            for field_id in fields {
                columns.push(table::column(
                    header(
                        global
                            .field(field_id)
                            .map(|f| f.display_name.as_str())
                            .unwrap_or("-")
                            .to_owned(),
                    ),
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

            scrollable(
                container(table(columns, &global.assets))
                    .padding(Padding::ZERO.bottom(10.).right(10.)),
            )
            .direction(scrollable::Direction::Both {
                horizontal: Default::default(),
                vertical: Default::default(),
            })
            .width(Length::Fill)
            .height(Length::Fill)
        };

        let sidebar = {
            let mut items = vec![
                checkbox(true).label("Tag").into(),
                checkbox(true).label("Category").into(),
                checkbox(true).label("Name").into(),
                space::vertical().height(5.).into(),
            ];

            for field in &global.fields {
                items.push(
                    checkbox(global.fetch_asset_fields.contains(&field.id))
                        .label(&field.display_name)
                        .on_toggle(|show| Message::ToggleFetchAssetField(field.id, show))
                        .into(),
                );
            }

            items.extend([
                space::vertical().height(10.).into(),
                button("Refresh").on_press(Message::RefreshAssets).into(),
            ]);

            column(items).padding(10.)
        };

        row![table, rule::vertical(1.), sidebar].into()
    }
}
