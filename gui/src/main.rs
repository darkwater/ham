use clap::Parser;
use eframe::egui;

use gui::state::{
    build_default_controller, grouped_assets_by_category, DefaultController, EventFormFieldKind,
};

struct GuiApp {
    controller: DefaultController,
}

impl GuiApp {
    fn new(base_url: &str) -> Self {
        let mut controller = build_default_controller(base_url);
        if let Err(err) = controller.load_catalog() {
            controller.state_mut().last_error = Some(err.to_string());
        }
        Self { controller }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.controller.state().clone();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Reload").clicked() {
                    if let Err(err) = self.controller.load_catalog() {
                        self.controller.state_mut().last_error = Some(err.to_string());
                    }
                }

                ui.label("Event Type");
                let mut event_type = state.event_type_draft.clone();
                ui.text_edit_singleline(&mut event_type);
                self.controller.state_mut().event_type_draft = event_type;

                if ui.button("New Event Form").clicked() {
                    let draft = self.controller.state().event_type_draft.clone();
                    if !draft.trim().is_empty() {
                        if let Err(err) = self.controller.start_event_from_type(draft.trim()) {
                            self.controller.state_mut().last_error = Some(err.to_string());
                        }
                    }
                }

                if let Some(err) = &state.last_error {
                    ui.colored_label(egui::Color32::RED, err);
                }
            });
        });

        egui::SidePanel::left("catalog").show(ctx, |ui| {
            ui.heading("Assets");
            let grouped = grouped_assets_by_category(&state.assets);
            for category in &state.categories {
                let count = grouped.get(&category.id).map_or(0, Vec::len);
                egui::CollapsingHeader::new(format!("{} ({count})", category.name))
                    .default_open(true)
                    .show(ui, |ui| {
                        if let Some(assets) = grouped.get(&category.id) {
                            for asset in assets {
                                let selected = state.selected_asset_tag.as_deref()
                                    == Some(asset.asset_tag.as_str());
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!(
                                            "{} {}",
                                            asset.asset_tag,
                                            asset.display_name.clone().unwrap_or_default()
                                        ),
                                    )
                                    .clicked()
                                {
                                    if let Err(err) =
                                        self.controller.open_asset_detail(&asset.asset_tag)
                                    {
                                        self.controller.state_mut().last_error =
                                            Some(err.to_string());
                                    }
                                }
                            }
                        }
                    });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Asset Detail");

            if let Some(tag) = &state.selected_asset_tag {
                ui.label(format!("Selected: {tag}"));
                ui.separator();

                ui.heading("Event Timeline");
                for item in &state.timeline {
                    ui.group(|ui| {
                        ui.label(format!(
                            "#{} {}@{}",
                            item.event_id, item.event_type_id, item.event_type_version
                        ));
                        ui.label(&item.timestamp);
                        ui.monospace(item.payload.to_string());
                    });
                }

                ui.separator();
                ui.heading("Apply Event");
                let mut key = state.idempotency_key_draft.clone();
                ui.horizontal(|ui| {
                    ui.label("Idempotency Key");
                    ui.text_edit_singleline(&mut key);
                });
                self.controller.state_mut().idempotency_key_draft = key;

                if let Some(form) = &state.event_form {
                    for field in &form.fields {
                        ui.horizontal(|ui| {
                            ui.label(&field.label);
                            let mut value = field.value.clone();
                            match &field.kind {
                                EventFormFieldKind::Enum(options) => {
                                    egui::ComboBox::from_id_source(format!(
                                        "enum-{}",
                                        field.input_key
                                    ))
                                    .selected_text(if value.is_empty() {
                                        "<select>"
                                    } else {
                                        value.as_str()
                                    })
                                    .show_ui(ui, |ui| {
                                        for option in options {
                                            ui.selectable_value(
                                                &mut value,
                                                option.option_key.clone(),
                                                format!(
                                                    "{} ({})",
                                                    option.display_name, option.option_key
                                                ),
                                            );
                                        }
                                    });
                                }
                                EventFormFieldKind::ExternalEntity(options) => {
                                    egui::ComboBox::from_id_source(format!(
                                        "external-{}",
                                        field.input_key
                                    ))
                                    .selected_text(if value.is_empty() {
                                        "<select>"
                                    } else {
                                        value.as_str()
                                    })
                                    .show_ui(ui, |ui| {
                                        for option in options {
                                            ui.selectable_value(
                                                &mut value,
                                                option.id.to_string(),
                                                format!("{} ({})", option.display_name, option.id),
                                            );
                                        }
                                    });
                                }
                                _ => {
                                    ui.text_edit_singleline(&mut value);
                                }
                            }
                            let _ = self.controller.set_form_value(&field.input_key, &value);
                        });
                    }

                    if ui.button("Apply").clicked() {
                        let key = self.controller.state().idempotency_key_draft.clone();
                        if key.trim().is_empty() {
                            self.controller.state_mut().last_error =
                                Some("idempotency key required".to_string());
                        } else if let Err(err) = self.controller.apply_event(key.trim()) {
                            self.controller.state_mut().last_error = Some(err.to_string());
                        }
                    }
                } else {
                    ui.label("Load an event type into form from top bar.");
                }
            } else {
                ui.label("Select an asset from the left panel.");
            }
        });
    }
}

#[derive(Debug, Parser)]
#[command(name = "gui", about = "Runs the HAM desktop GUI")]
struct GuiCli;

fn main() -> eframe::Result<()> {
    let _ = domain::domain_ready();

    let _cli = GuiCli::parse();

    let base_url = std::env::var("HAM_SERVER_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "HAM Asset Tracker GUI",
        options,
        Box::new(move |_cc| Box::new(GuiApp::new(&base_url))),
    )
}

#[cfg(test)]
mod tests {
    use super::GuiCli;
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn parse_accepts_no_flags() {
        let parsed = GuiCli::try_parse_from(["gui"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn parse_rejects_unknown_flags() {
        let err = GuiCli::try_parse_from(["gui", "--windowed"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn parse_supports_help_flag() {
        let err = GuiCli::try_parse_from(["gui", "-h"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }
}
