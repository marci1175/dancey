use std::{collections::HashMap, ops::Add, sync::Arc};

use crate::ui::panels::lib::Panel;
use egui::{
    Align2, Color32, FontId, InnerResponse, Popup, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
    Widget,
};
use parking_lot::RwLock;

const TRACK_LABEL: Color32 = Color32::ORANGE;
const TRACK_LABEL_TEXT: Color32 = Color32::WHITE;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TrackCustomization {
    pub label_text: String,
    pub label_text_color: Color32,
    pub label_color: Color32,
}

impl TrackCustomization {
    fn named_default(label_text: String) -> Self {
        Self {
            label_text,
            label_text_color: TRACK_LABEL_TEXT,
            label_color: TRACK_LABEL,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistState {
    /// Can be modified with the bpm slider.
    pub bpm: f32,

    /// Indicates the position of the cursor.
    pub cursor_offset: f32,

    /// This indicates how much the user has scrolled.
    pub grid_offset: Vec2,

    /// Track customization
    pub custom_tracks: HashMap<usize, TrackCustomization>,
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

    ui.painter_at(playlist_rect)
        .rect_filled(ui.available_rect_before_wrap(), 0., Color32::BLACK);

    // Set the height of the tracks (the horizontal space between two lines in the "grid")
    const TRACK_HEIGHT: usize = 100;
    const BEAT_WIDTH: usize = 25;

    // Colors
    const BAR_TRACK_SEPARATOR: Color32 = Color32::GRAY;

    const STROKE_WIDTH: f32 = 1.0f32;
    const CURSOR: Color32 = Color32::LIGHT_GREEN;

    // The total grid's offset (the amount the user has scrolled.)
    let grid_offset = state.read().grid_offset.clone();
    let normalized_y_offset = grid_offset.y / playlist_rect.height();
    let normalized_x_offset = grid_offset.x / playlist_rect.width();

    // This indicates that the track label is 4 bars wide
    const TRACK_LABEL_WIDTH: usize = BEAT_WIDTH * 4;

    // Draw beats
    // We skip two of the positions, because the first one would be off-screen and the second one gets the track label instead
    for x_coord in ((playlist_rect.left() + normalized_x_offset) as i32
        ..(playlist_rect.right() - normalized_x_offset) as i32)
        .step_by(BEAT_WIDTH)
        .skip(TRACK_LABEL_WIDTH / BEAT_WIDTH)
    {
        ui.painter().line(
            vec![
                Pos2::new(
                    (x_coord as f32 + normalized_x_offset)
                        .clamp(playlist_rect.left(), playlist_rect.right()),
                    playlist_rect.top(),
                ),
                Pos2::new(
                    (x_coord as f32 + normalized_x_offset)
                        .clamp(playlist_rect.left(), playlist_rect.right()),
                    playlist_rect.bottom(),
                ),
            ],
            Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
        );
    }

    // Draw track labels (filled rect)
    // This rectangle takes up four bar widths
    for (idx, y_coord) in ((playlist_rect.top() + normalized_y_offset) as i32
        ..(playlist_rect.bottom() - normalized_y_offset) as i32)
        .step_by(TRACK_HEIGHT)
        .enumerate()
    {
        let label_text = format!("Track {idx}");

        // Try getting the customization state for the current label
        let label_customization = match state.read().custom_tracks.get(&idx) {
            Some(custom) => custom.clone(),
            None => TrackCustomization::named_default(label_text.clone()),
        };

        let top = (y_coord as f32 + normalized_y_offset).max(playlist_rect.top());
        let bottom = (y_coord as f32 + normalized_y_offset + TRACK_HEIGHT as f32)
            .min(playlist_rect.bottom());

        if top >= playlist_rect.bottom() || bottom <= playlist_rect.top() {
            continue;
        }

        let label_rect = Rect::from_two_pos(
            Pos2 {
                x: playlist_rect.left(),
                y: top,
            },
            Pos2 {
                x: playlist_rect.left() + TRACK_LABEL_WIDTH as f32,
                y: bottom,
            },
        );

        // Draw the label itself
        ui.painter()
            .rect_filled(label_rect, 0., label_customization.label_color);

        // Draw the label text
        ui.painter().text(
            label_rect.center(),
            Align2::CENTER_TOP,
            label_customization.label_text.clone(),
            FontId::default(),
            label_customization.label_text_color,
        );

        // Allocate the response for the given track
        let label = ui.allocate_rect(label_rect, Sense::click());

        // Get access to the track customizations
        let custom_tracks = &mut state.write().custom_tracks;

        // Detect if it has been right clicked on and store a entry in the customization list.
        if label.secondary_clicked() && !custom_tracks.contains_key(&idx) {
            custom_tracks.insert(idx, TrackCustomization::named_default(label_text.clone()));
        }

        if custom_tracks.contains_key(&idx) {
            // Get mutable access to the created item
            // Its safe to unwrap here due to the check above
            let customization_state = custom_tracks.get_mut(&idx).unwrap();
            // Open ctx menu and access the entry weve created
            let popup = egui::Popup::context_menu(&label)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);

            let ctx_menu = popup.show(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::from("Label").weak());
                    ui.text_edit_singleline(&mut customization_state.label_text);
                });
                ui.separator();
                ui.label(RichText::from("Label Color"));
                egui::widgets::color_picker::color_picker_color32(
                    ui,
                    &mut customization_state.label_color,
                    egui::widgets::color_picker::Alpha::Opaque,
                );
                ui.separator();
                ui.label(RichText::from("Label Text Color"));
                egui::widgets::color_picker::color_picker_color32(
                    ui,
                    &mut customization_state.label_text_color,
                    egui::widgets::color_picker::Alpha::Opaque,
                );
            });

            // Check if the user has clicked outside of the context menu 
            if ctx_menu.is_none() {
                // If the context menu is closed we should check if the customization entry has been modified
                // If not just remove it to save up memory
                if &*customization_state == &TrackCustomization::named_default(label_text) {
                    custom_tracks.remove(&idx);
                }
            }
        }
    }

    // Draw track separator lines
    for y_coord in ((playlist_rect.top() + normalized_y_offset) as i32
        ..(playlist_rect.bottom() - normalized_y_offset) as i32)
        .step_by(TRACK_HEIGHT)
    {
        ui.painter().line(
            vec![
                Pos2::new(
                    playlist_rect.left(),
                    (y_coord as f32 + normalized_y_offset)
                        .clamp(playlist_rect.top(), playlist_rect.bottom()),
                ),
                Pos2::new(
                    playlist_rect.right(),
                    (y_coord as f32 + normalized_y_offset)
                        .clamp(playlist_rect.top(), playlist_rect.bottom()),
                ),
            ],
            Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
        );
    }

    // Get cursor position (offest)
    let cursor_offset = state.read().cursor_offset;

    // Draw main cursor (Indicates where we are in current playlist)
    ui.painter().line(
        vec![
            Pos2::new(
                (playlist_rect.left() + cursor_offset + grid_offset.x).min(playlist_rect.right()),
                playlist_rect.top(),
            ),
            Pos2::new(
                (playlist_rect.left() + cursor_offset + grid_offset.x).min(playlist_rect.right()),
                playlist_rect.bottom(),
            ),
        ],
        Stroke::new(STROKE_WIDTH, CURSOR),
    );

    // We are going to have multiple layers of responses each capturing something different
    // The main playlist response should capture scrolling input in order to offset the whole grid
    let ui_base = ui.allocate_rect(playlist_rect, Sense::hover());

    // Capture scroll if hovered
    if ui_base.hovered() {
        let scroll_delta = ui.input(|reader| reader.smooth_scroll_delta());
        state.write().grid_offset = grid_offset.add(scroll_delta * 200.).min(Vec2::default());
    }
}
