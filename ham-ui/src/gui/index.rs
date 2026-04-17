use ham_shared::CategoryId;

use crate::gui::GlobalState;

#[derive(Debug, Default)]
pub struct Index {
    category_asset_counts: Vec<(CategoryId, usize)>,
}

impl Index {
    pub fn calculate(global: &GlobalState) -> Self {
        Self {
            category_asset_counts: global
                .categories
                .iter()
                .map(|cat| {
                    let count = global
                        .assets
                        .iter()
                        .filter(|asset| asset.category_id == cat.id)
                        .count();
                    (cat.id, count)
                })
                .collect(),
        }
    }

    pub fn category_asset_count(&self, category_id: CategoryId) -> Option<usize> {
        self.category_asset_counts
            .iter()
            .find(|(cat_id, _)| *cat_id == category_id)
            .map(|(_, count)| *count)
    }
}
