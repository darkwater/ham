// use std::ops::{Deref, DerefMut};

use std::sync::Arc;

use egui::mutex::Mutex;
use tokio::sync::mpsc::UnboundedSender;

pub struct ElmCtx<M> {
    pub(crate) queue: UnboundedSender<M>,
}

// impl<'a, M> Deref for ElmCtx<'a, M> {
//     type Target = egui::Ui;

//     fn deref(&self) -> &Self::Target {
//         self.egui
//     }
// }

// impl<'a, M> DerefMut for ElmCtx<'a, M> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.egui
//     }
// }

impl<M> ElmCtx<M> {
    // pub fn egui(&mut self) -> &mut egui::Ui {
    //     self.egui
    // }

    // pub fn frame(&mut self) -> &mut eframe::Frame {
    //     self.frame
    // }

    pub fn send(&mut self, message: M) {
        self.queue.send(message).expect("Message queue was closed");
    }
}

impl<M> ElmCtx<M> {
    // pub fn map_ui<'ui: 'a, T>(
    //     &mut self,
    //     map: impl FnOnce(
    //         &'ui mut egui::Ui,
    //         Box<dyn FnOnce(&'ui mut egui::Ui) -> T>,
    //     ) -> egui::InnerResponse<T>
    //     + 'static,
    //     inner: impl FnOnce(ElmUi<'ui, M>) -> T + 'static,
    // ) -> egui::InnerResponse<T> {
    //     let Self { egui, frame, queue } = self;

    //     map(
    //         egui,
    //         Box::new(|ui| {
    //             let mut inner_ui = Self {
    //                 egui: ui,
    //                 frame,
    //                 queue: queue.clone(),
    //             };
    //             inner(inner_ui)
    //         }),
    //     )
    // }
}

pub trait EguiUiExt {
    fn hold_value<T>(&self, id: impl Into<egui::Id>, default: &T) -> Arc<Mutex<T::Owned>>
    where
        T: ?Sized + ToOwned,
        T::Owned: Send + 'static;
}

impl EguiUiExt for egui::Ui {
    fn hold_value<T>(&self, id: impl Into<egui::Id>, default: &T) -> Arc<Mutex<T::Owned>>
    where
        T: ?Sized + ToOwned,
        T::Owned: Send + 'static,
    {
        self.memory_mut(|mem| {
            mem.data
                .get_temp_mut_or_insert_with(id.into(), || Arc::new(Mutex::new(default.to_owned())))
                .clone()
        })
    }
}
