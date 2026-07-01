use egui::{InnerResponse, Ui};



pub fn display_settings_window(ui: &mut Ui) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Settings").show(ui.ctx(), |ui| {
        
    })
}