use ham_shared::{Category, CategoryId};
use iced::{Length, Task};

use super::{GlobalState, Message};

#[derive(Debug, Default)]
pub struct State {
    selected_category: Option<CategoryId>,
}

#[derive(Debug, Clone)]
pub enum CategoriesMessage {
    SelectCategory(CategoryId),
}

impl State {
    pub fn update(&mut self, message: CategoriesMessage) -> Task<Message> {
        match message {
            CategoriesMessage::SelectCategory(id) => {
                self.selected_category = Some(id);
            }
        }

        Task::none()
    }

    pub fn view<'a>(&'a self, global: &'a GlobalState) -> iced::Element<'a, Message> {
        use iced::widget::*;

        let cat_list = column(
            global
                .categories
                .iter()
                .map(|category| {
                    let label = CategoryAncestryIter { global, current: Some(category) }.display();

                    radio(label, category.id, self.selected_category, |id| {
                        Message::CategoriesPage(CategoriesMessage::SelectCategory(id))
                    })
                    .into()
                })
                .intersperse_with(|| space::vertical().height(5.).into())
                .collect::<Vec<_>>(),
        )
        .padding(10.)
        .width(Length::FillPortion(1));

        if let Some(cat_id) = self.selected_category {
            let Some(category) = global.category(cat_id) else {
                return text("Selected category is gone").into();
            };

            let details = column([text(cat_id.to_string()).into()])
                .padding(10.)
                .width(Length::FillPortion(1));

            container(row![cat_list, rule::vertical(1.), details])
        } else {
            container(cat_list)
        }
        .into()
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

impl CategoryAncestryIter<'_> {
    fn display(self) -> String {
        let mut ancestry = self.map(|c| c.display_name.as_str()).collect::<Vec<_>>();
        ancestry.reverse();
        ancestry.join(" ⏵ ")
    }
}

// #[derive(Clone)]
// struct CategoryOption<'a> {
//     global: &'a GlobalState,
//     category: &'a Category,
// }

// impl Hash for CategoryOption<'_> {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.category.id.hash(state);
//     }
// }

// impl Eq for CategoryOption<'_> {}
// impl PartialEq for CategoryOption<'_> {
//     fn eq(&self, other: &Self) -> bool {
//         self.category.id == other.category.id
//     }
// }

// impl CategoryOption<'_> {
//     fn id(&self) -> CategoryId {
//         self.category.id
//     }
// }

// impl Display for CategoryOption<'_> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let mut ancestry = CategoryAncestryIter {
//             global: self.global,
//             current: Some(self.category),
//         }
//         .map(|c| c.display_name.as_str())
//         .collect::<Vec<_>>();

//         ancestry.reverse();

//         write!(f, "{}", ancestry.join(" ⏵ "))
//     }
// }
