use std::{ops::Add, sync::Arc};

use egui::{Color32, InnerResponse, Pos2, Stroke, Ui};
use parking_lot::RwLock;
use crate::ui::panels::lib::Panel;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistState {
    pub bpm: f32,
    pub cursor_offset: f32,
}

pub fn playlist_ui(this: &Panel, ui: &mut Ui, state: Arc<RwLock<PlaylistState>>) {
    // Draw the main options / tools for this ui
    ui.horizontal(|ui| {
        ui.button("Start");
        ui.button("Pause");
        ui.button("Stop");
        ui.button("Patterns");

        ui.add(egui::Slider::new(&mut state.write().bpm, 10.0..=522.0));
    });

    ui.separator();

    // Paint the background black, and draw on top of that
    let playlist_rect = ui.available_rect_before_wrap();
    ui.painter_at(playlist_rect).rect_filled(
        ui.available_rect_before_wrap(),
        0.,
        Color32::BLACK,
    );

    // Set the height of the tracks (the horizontal space between two lines in the "grid")
    const TRACK_HEIGHT: usize = 100;
    const BEAT_WIDTH: usize = 25;
    
    // Draw worklines
    for y_coord in (0..=playlist_rect.height() as i32).step_by(TRACK_HEIGHT).skip(1) {
        ui.painter().line(vec![Pos2::new(playlist_rect.left(), y_coord as f32), Pos2::new(playlist_rect.right(), y_coord as f32)], Stroke::new(2.0f32, Color32::GRAY));
    }
    for x_coord in (0..=playlist_rect.right() as i32).step_by(BEAT_WIDTH).skip(1) {
        ui.painter().line(vec![Pos2::new(x_coord as f32, playlist_rect.top()), Pos2::new(x_coord as f32, playlist_rect.bottom())], Stroke::new(2.0f32, Color32::GRAY));
    }

    // Get cursor position (offest)
    let cursor_offset = state.read().cursor_offset;

    // Draw main cursor
    ui.painter().line(vec![Pos2::new(playlist_rect.left().add(cursor_offset), playlist_rect.top()), Pos2::new(playlist_rect.left().add(cursor_offset), playlist_rect.bottom())], Stroke::new(2.0f32, Color32::LIGHT_GREEN));
    
    state.write().cursor_offset += 1.0;
}
