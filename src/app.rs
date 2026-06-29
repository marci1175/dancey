use std::sync::Arc;

use eframe::{App, CreationContext};

use crate::ui::panels::lib::{Panel, PanelStates, create_panels};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Application {
    /// The state of the panels inside, every panel state is accessible from the other one.
    panel_states: Arc<PanelStates>,

    /// The list of panels that are present in the application.
    panels: Vec<Panel>,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            // Store the state of the panels separately
            panel_states: Arc::new(PanelStates::default()),

            // Complete list of all of the panels of the application
            panels: create_panels(),
        }
    }
}

impl Application {
    pub fn new(cc: &CreationContext) -> Self {
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl App for Application {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {}

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Create the main options bar
        egui::Panel::top("application_options").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |_ui| {});
                ui.menu_button("View", |_ui| {});
                ui.menu_button("Plugins", |_ui| {});
                ui.menu_button("Help", |_ui| {});
                ui.menu_button("About", |_ui| {});
            });
        });

        // Draw detachable panels
        for panel in self.panels.iter() {
            // Draw/update panel
            panel.display(ui, self.panel_states.clone());

            // If the panel is not detached we can display its toasts in the root ui
            if !panel.detached.load(std::sync::atomic::Ordering::Relaxed) {
                panel.toasts.lock().show(ui);
            }
        }
    }
}
