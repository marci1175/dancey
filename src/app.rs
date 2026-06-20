use eframe::{App, CreationContext};
use egui::{CentralPanel, ViewportBuilder, ViewportId};

use crate::ui::panels::lib::{create_panels, Panel, PanelId};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Application {
    panels: Vec<Panel>,
}

impl Default for Application {
    fn default() -> Self {
        Self {
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

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        for panel in self.panels.iter_mut() {
            panel.display(ctx);
        }
    }
}
