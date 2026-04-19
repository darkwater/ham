use std::{clone::Clone, sync::Arc};

use egui::mutex::Mutex;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy)]
pub struct ElmCtx<'a, M> {
    pub(crate) queue: &'a UnboundedSender<M>,
}

impl<M> ElmCtx<'_, M> {
    pub fn send(&self, message: M) {
        self.queue.send(message).expect("Message queue was closed");
    }

    // /// return true if you changed the value
    // pub fn hold_value<T: ToOwned>(
    //     &self,
    //     id: impl Into<egui::Id>,
    //     default: &T,
    //     scope: impl FnOnce(HoldValue<T>) -> bool,
    // ) {
    //     let id = id.into();
    //     let value = self
    //         .hold_values
    //         .get(&id)
    //         .and_then(|boxed| boxed.downcast_ref::<Arc<Mutex<T::Owned>>>())
    //         .cloned()
    //         .unwrap_or_else(|| Arc::new(Mutex::new(default.to_owned())));

    //     let hold_value = HoldValue { value: Cow::Borrowed(&*value.lock()) };
    //     if scope(hold_value) {
    //         *value.lock() = hold_value.value.into_owned();
    //     }
    // }
}

// pub struct HoldValue<'a, T: Clone> {
//     value: Cow<'a, T>,
// }

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
