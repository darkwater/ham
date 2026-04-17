mod app;
mod ui;

use std::{future::Future, pin::Pin};

pub use app::*;
pub use ui::*;

pub struct Task<T: Send + 'static> {
    futures: Vec<Pin<Box<dyn Future<Output = T> + Send>>>,
}

impl<M: Send + 'static> Task<M> {
    pub fn none() -> Self {
        Self { futures: Vec::new() }
    }

    pub fn perform<T>(
        fut: impl Future<Output = T> + Send + 'static,
        map: impl FnOnce(T) -> M + Send + 'static,
    ) -> Self {
        Self {
            futures: vec![Box::pin(async move { map(fut.await) })],
        }
    }

    pub fn multiple(tasks: impl IntoIterator<Item = Self>) -> Self {
        Self {
            futures: tasks.into_iter().flat_map(|t| t.futures).collect(),
        }
    }
}

pub trait Fragment {
    type Message: Send + 'static;

    fn init(cc: &eframe::CreationContext) -> (Self, Task<Self::Message>)
    where
        Self: Sized;

    fn update(&mut self, message: Self::Message, ctx: &egui::Context) -> Task<Self::Message>;

    fn view(&self, ui: &mut egui::Ui, frame: &mut eframe::Frame, elm: ElmCtx<Self::Message>);
}
