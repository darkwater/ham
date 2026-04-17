use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{ElmCtx, Fragment, Task};

pub trait App: Fragment {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        _ = storage;
    }

    fn on_exit(&mut self) {}

    fn auto_save_interval(&self) -> Duration {
        Duration::from_secs(30)
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.window_fill().to_normalized_gamma_f32()
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        _ = (ctx, raw_input);
    }
}

pub fn run_app<A: App + 'static>(
    app_name: &str,
    options: eframe::NativeOptions,
) -> eframe::Result<()> {
    eframe::run_native(app_name, options, Box::new(|cc| Ok(Box::new(Runtime::<A>::new(cc)?))))
}

struct Runtime<A: App> {
    app: A,
    rx: UnboundedReceiver<A::Message>,
    tx: UnboundedSender<A::Message>,
    egui_ctx: egui::Context,
    runtime: tokio::runtime::Runtime,
}

impl<A: App> Runtime<A> {
    fn new(cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (app, task) = A::init(cc);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let runtime = tokio::runtime::Runtime::new()?;

        let egui_ctx = cc.egui_ctx.clone();

        let this = Self { app, tx, rx, egui_ctx, runtime };

        this.spawn_task(task);

        Ok(this)
    }

    fn spawn_task(&self, task: Task<A::Message>) {
        for mut future in task.futures {
            let tx = self.tx.clone();
            let egui_ctx = self.egui_ctx.clone();
            self.runtime.spawn(async move {
                let msg = future.as_mut().await;
                let _ = tx.send(msg); // TODO: maybe log err?

                egui_ctx.request_repaint();
            });
        }
    }
}

impl<A: App> eframe::App for Runtime<A> {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(message) = self.rx.try_recv() {
            let task = self.app.update(message, ctx);
            self.spawn_task(task);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let elm = ElmCtx {
            // egui: ui,
            // frame,
            queue: self.tx.clone(),
        };

        self.app.view(ui, frame, elm);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.save(storage);
    }

    fn on_exit(&mut self) {
        self.app.on_exit();
    }

    fn auto_save_interval(&self) -> Duration {
        self.app.auto_save_interval()
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }

    fn persist_egui_memory(&self) -> bool {
        self.app.persist_egui_memory()
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw_input);
    }
}
