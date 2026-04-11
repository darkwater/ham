use std::hash::Hash;

use serde::{Deserialize, Serialize};

use super::{Asset, AssetId, Category, CategoryId, Field, FieldId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Map<T: MapType> {
    inner: Vec<T>,
}

impl<T: MapType> Default for Map<T> {
    fn default() -> Self {
        Self { inner: Default::default() }
    }
}

impl<T: MapType> Map<T> {
    pub fn push(&mut self, item: T) {
        self.inner.push(item);
    }

    pub fn get(&self, key: T::Key) -> Option<&T> {
        self.inner.iter().find(|item| item.key() == key)
    }

    pub fn get_mut(&mut self, key: T::Key) -> Option<&mut T> {
        self.inner.iter_mut().find(|item| item.key() == key)
    }

    pub fn get_index(&self, idx: usize) -> Option<&T> {
        self.inner.get(idx)
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.inner.iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.inner.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn remove(&mut self, id: T::Key) -> Option<T> {
        let idx = self.inner.iter().position(|item| item.key() == id)?;
        Some(self.inner.remove(idx))
    }
}

pub trait MapType {
    type Key: Copy + Eq + Hash;

    fn key(&self) -> Self::Key;
}

impl MapType for Asset {
    type Key = AssetId;

    fn key(&self) -> Self::Key {
        self.id
    }
}

impl MapType for Category {
    type Key = CategoryId;

    fn key(&self) -> Self::Key {
        self.id
    }
}

impl MapType for Field {
    type Key = FieldId;

    fn key(&self) -> Self::Key {
        self.id
    }
}
