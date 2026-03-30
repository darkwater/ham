use core::future::pending;
use std::sync::RwLock;

use poll_promise::Promise;

pub static QUEUED: RwLock<Option<QueueRefresh>> = RwLock::new(None);
pub enum QueueRefresh {
    Assets,
    Categories,
}

pub struct RemoteState {
    pub assets: Promise<Vec<ham_shared::assets::Asset>>,
    pub categories: Promise<Vec<ham_shared::categories::Category>>,
}

impl RemoteState {
    pub fn new() -> Self {
        let mut this = Self {
            assets: Promise::spawn_async(pending()),
            categories: Promise::spawn_async(pending()),
        };

        this.refresh_assets();
        this.refresh_categories();

        this
    }

    pub fn poll_refresh(&mut self) {
        if let Some(queue) = QUEUED.write().unwrap().take() {
            match queue {
                QueueRefresh::Assets => self.refresh_assets(),
                QueueRefresh::Categories => self.refresh_categories(),
            }
        }
    }

    pub fn refresh_assets(&mut self) {
        self.assets = Promise::spawn_async(async move {
            surf::get("http://localhost:6172/assets")
                .recv_json()
                .await
                .unwrap_or_default()
        });
    }

    pub fn refresh_categories(&mut self) {
        self.categories = Promise::spawn_async(async move {
            surf::get("http://localhost:6172/categories")
                .recv_json()
                .await
                .unwrap_or_default()
        });
    }

    pub fn assets(&self) -> &[ham_shared::assets::Asset] {
        self.assets
            .ready()
            .map(|v| v.as_slice())
            .unwrap_or_default()
    }

    pub fn categories(&self) -> &[ham_shared::categories::Category] {
        self.categories
            .ready()
            .map(|v| v.as_slice())
            .unwrap_or_default()
    }

    pub fn category(&self, id: i64) -> Option<&ham_shared::categories::Category> {
        self.categories().iter().find(|c| c.id == id)
    }

    pub fn asset(&self, id: i64) -> Option<&ham_shared::assets::Asset> {
        self.assets().iter().find(|a| a.id == id)
    }
}
