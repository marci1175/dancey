use std::{collections::HashMap, ops::Add, sync::Arc};

use crate::ui::panels::lib::Panel;
use egui::{
    Align2, Color32, FontId, InnerResponse, Popup, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, Widget, vec2,
};
use parking_lot::RwLock;

const TRACK_LABEL: Color32 = Color32::ORANGE;
const TRACK_LABEL_TEXT: Color32 = Color32::WHITE;
const TRACK_HEIGHT: f32 = 100.0;
const MINIMUM_TRACK_HEIGHT: f32 = 10.;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TrackCustomization {
    pub label_text: String,
    pub label_text_color: Color32,
    pub label_color: Color32,
    pub height: f32,

    /// This just makes it so that if the track's height has ever been set this will be true and it wont be automatically deleted
    pub height_set: bool,
}

impl TrackCustomization {
    fn named_default(label_text: String) -> Self {
        Self {
            label_text,
            label_text_color: TRACK_LABEL_TEXT,
            label_color: TRACK_LABEL,
            height: TRACK_HEIGHT,
            
            height_set: false,
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

    let mut current_height = playlist_rect.top() + normalized_y_offset;
    let mut idx = 0;
    let max_height = playlist_rect.bottom() - normalized_y_offset;

    // Draw track labels (filled rect) and track separator lines
    // This rectangle takes up four bar widths
    while current_height < max_height {
        let label_text = format!("Track {idx}");
        let y_coord = current_height;

        // Try getting the customization state for the current label
        let label_customization = match state.read().custom_tracks.get(&idx) {
            Some(custom) => custom.clone(),
            None => TrackCustomization::named_default(label_text.clone()),
        };

        let top = (y_coord as f32 + normalized_y_offset).max(playlist_rect.top());
        let bottom = (y_coord as f32 + normalized_y_offset + label_customization.height)
            .min(playlist_rect.bottom());

        let is_visible = !(top >= playlist_rect.bottom() || bottom <= playlist_rect.top());

        if is_visible {
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

            // We should only allow the context menu to be opened if we already have the track customizations saved in the list.
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
                    if &*customization_state == &TrackCustomization::named_default(label_text.clone()) {
                        custom_tracks.remove(&idx);
                    }
                }
            }

            // Draw track separator lines
            let separator_points = vec![
                Pos2::new(
                    playlist_rect.left(),
                    (y_coord as f32 + normalized_y_offset + label_customization.height)
                        .clamp(playlist_rect.top(), playlist_rect.bottom()),
                ),
                Pos2::new(
                    playlist_rect.right(),
                    (y_coord as f32 + normalized_y_offset + label_customization.height)
                        .clamp(playlist_rect.top(), playlist_rect.bottom()),
                ),
            ];

            ui.painter().line(
                separator_points.clone(),
                Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
            );

            // Allocate a response for being able to set the height of the tracks
            let separator = ui.allocate_rect(Rect::from_points(&separator_points).expand2(vec2(0., 2.5)), Sense::click_and_drag());
            
            // Get how much this has been dragged by
            let height_delta = separator.drag_delta().y;
            let pixel_delta = ui.pixels_per_point() * height_delta;

            // Check if a drag has been started
            if separator.drag_started() && !custom_tracks.contains_key(&idx) {
                custom_tracks.insert(idx, TrackCustomization::named_default(label_text.clone()));
            }

            // Check if the item is inside the list
            if custom_tracks.contains_key(&idx) {
                // Get mutable access to the created item
                // Its safe to unwrap here due to the check above
                let customization_state = custom_tracks.get_mut(&idx).unwrap();
                
                // Set that it has been modified already
                customization_state.height_set = true;

                // If it has been double clicked that means that it should minimize the track or if its already minimzed then reset it to the original value
                if separator.double_clicked() {
                    if customization_state.height != MINIMUM_TRACK_HEIGHT {
                        customization_state.height = MINIMUM_TRACK_HEIGHT;
                    }
                    else {
                        customization_state.height = TRACK_HEIGHT;
                    }
                }
                else {
                    customization_state.height = customization_state
                        .height
                        .add(pixel_delta)
                        .max(MINIMUM_TRACK_HEIGHT);
                }
            }

            // Indicate that this can be grabbed
            separator.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        }

        // Add the consumed height to the current height
        current_height += label_customization.height;

        // Track indexes too
        idx += 1;
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
