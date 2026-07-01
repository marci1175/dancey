use std::{path::PathBuf, sync::Arc};

use eframe::{App, CreationContext};
use egui::{RichText, vec2};

use crate::{
    IS_DEBUG, project_manager::open_project, ui::{panels::lib::{Panel, PanelStates, create_panels}, windows::WindowsManager},
};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Application {
    /// The state of the panels inside, every panel state is accessible from the other one.
    panel_states: Arc<PanelStates>,

    /// The list of panels that are present in the application.
    panels: Vec<Panel>,

    /// Recently opened project's paths
    recently_opened: Vec<PathBuf>,

    /// If the user has saved a project or opened an existing one this path will point to that file which has been opened.
    save_path: Option<PathBuf>,

    /// This field indicates which floating windows are enabled (visible).
    #[serde(skip)]
    opened_windows: WindowsManager,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            // Store the state of the panels separately
            panel_states: Arc::new(PanelStates::default()),

            // Complete list of all of the panels of the application
            panels: create_panels(),

            // Recently opened project paths
            recently_opened: Vec::new(),

            // If no paths were logged then this should be None.
            save_path: None,

            // A struct indicating which windows are enabled
            opened_windows: WindowsManager::default(),
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
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {}

                    ui.separator();

                    if ui.button("Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Beatroot Project", &["btrt"])
                            .pick_file()
                        {
                            open_project(&path);
                        }
                    }
                    ui.menu_button("Open Recent", |ui| {
                        ui.allocate_ui(vec2(250., 0.), |ui| {
                            ui.label("Recent Projects");
                            ui.separator();
                            for (idx, path) in self.recently_opened.iter().enumerate() {
                                if ui
                                    .button(RichText::from(format!("{idx}. {}", path.display())))
                                    .clicked()
                                {
                                    open_project(path);
                                }
                            }
                        });
                    });

                    ui.separator();

                    if ui.button("Save As").clicked() {}
                    if ui.button("Save").clicked() {}
                });

                ui.menu_button("View", |_ui| {});

                if ui.button("Plugins").clicked() {}

                if ui.button("Settings").clicked() {}

                ui.menu_button("Help", |ui| {
                    ui.label("Build information");
                    ui.label(format!("Build: {}{}", env!("CARGO_PKG_VERSION"), {
                        if IS_DEBUG { "debug" } else { "release" }
                    }));
                    ui.separator();
                    ui.hyperlink_to("API documentation", "https://www.google.com")
                });

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
