use std::{marker::PhantomData, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommaSeparated<T> {
    str: String,
    _phantom: PhantomData<T>,
}

impl<T> Serialize for CommaSeparated<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.str.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for CommaSeparated<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        Ok(Self { str, _phantom: PhantomData })
    }
}

impl<T> CommaSeparated<T> {
    pub fn from_slice(slice: &[T]) -> Self
    where
        T: ToString,
    {
        let str = slice
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");

        Self { str, _phantom: PhantomData }
    }

    pub fn as_str(&self) -> &str {
        &self.str
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: FromStr,
    {
        self.str.split(',').filter_map(|s| s.parse().ok()).collect()
    }
}
