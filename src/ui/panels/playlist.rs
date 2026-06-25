use std::{collections::HashMap, ffi::OsString, ops::Add, sync::Arc};

use crate::{internals::sample::SampleProperties, ui::panels::lib::Panel};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, vec2};
use parking_lot::RwLock;

const TRACK_LABEL: Color32 = Color32::ORANGE;
const TRACK_LABEL_TEXT: Color32 = Color32::WHITE;
const TRACK_HEIGHT: f32 = 100.0;
const MINIMUM_TRACK_HEIGHT: f32 = 10.;

// Set the height of the tracks (the horizontal space between two lines in the "grid")
const BEAT_WIDTH: usize = 25;

// Colors
const BAR_TRACK_SEPARATOR: Color32 = Color32::GRAY;
const STROKE_WIDTH: f32 = 1.0f32;
const CURSOR_COLOR: Color32 = Color32::LIGHT_GREEN;

// This indicates that the track label is 4 bars wide
const TRACK_LABEL_WIDTH: usize = BEAT_WIDTH * 4;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TrackCustomization {
    pub label_text: String,
    pub label_text_color: Color32,
    pub label_color: Color32,
    pub height: f32,

    /// This just makes it so that if the track's height has ever been set this will be true and it wont be automatically deleted
    pub height_set: bool,
}

pub struct DNDSampleInstance {
    pub name: OsString,
    pub color: Color32,
    pub properties: SampleProperties,
}

impl TrackCustomization {
    fn named_default(nth: usize) -> Self {
        Self {
            label_text: format!("Track {nth}"),
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

    #[serde(skip)]
    /// Indicates the position of the cursor.
    pub cursor_offset: f32,

    /// This indicates how much the user has scrolled.
    pub grid_offset: Vec2,

    /// Track customization
    pub custom_tracks: HashMap<usize, TrackCustomization>,
}

const BPM_PRESETS: &[f32] = &[
    60.0, 70.0, 80.0, 90.0, 100.00, 110.0, 120.0, 128.0, 140.0, 165.0, 174.0,
];

pub fn playlist_ui(_this: &Panel, ui: &mut Ui, state: Arc<RwLock<PlaylistState>>) {
    // Draw the main options / tools for this ui
    ui.horizontal(|ui| {
        ui.button("Start");
        ui.button("Pause");
        ui.button("Stop");
        ui.button("Patterns");

        ui.label("bpm");

        let playlist_bpm = &mut state.write().bpm;
        ui.add(egui::Slider::new(playlist_bpm, 10.0..=522.0).fixed_decimals(3))
            .context_menu(|ui| {
                ui.label("Presets");

                ui.separator();

                for bpm in BPM_PRESETS {
                    if ui.button(format!("{bpm} bpm")).clicked() {
                        *playlist_bpm = *bpm;
                    }
                }
            });
    });

    ui.separator();

    // Paint the background black, and draw on top of that
    let playlist_rect = ui.available_rect_before_wrap();

    ui.painter_at(playlist_rect)
        .rect_filled(ui.available_rect_before_wrap(), 0., Color32::BLACK);

    // The total grid's offset (the amount the user has scrolled.)
    let grid_offset = state.read().grid_offset;
    let normalized_y_offset = grid_offset.y / playlist_rect.height();
    let normalized_x_offset = grid_offset.x / playlist_rect.width();

    // Track the positions of the lines drawn so that we can visualize the preview of a sample in the playlist.
    let beat_lines = beat_outlines(ui, playlist_rect, normalized_x_offset, BEAT_WIDTH as f32);
    let mut track_lines = Vec::new();

    let mut current_height = playlist_rect.top() + normalized_y_offset;
    let mut idx = 0;
    let max_height = playlist_rect.bottom() - normalized_y_offset;

    let mut last_visible_track_idx = 0;
    let mut is_first_track_visible = false;

    // Draw track labels (filled rect) and track separator lines
    // This rectangle takes up four bar widths
    while current_height < max_height {
        let y_coord = current_height;

        // Try getting the customization state for the current label
        let label_customization = get_track_customization(state.clone(), idx);

        let top = (y_coord + normalized_y_offset).max(playlist_rect.top());
        let bottom = (y_coord + normalized_y_offset + label_customization.height)
            .min(playlist_rect.bottom());

        let is_visible = !(top >= playlist_rect.bottom() || bottom <= playlist_rect.top());

        // This will always be set to the last visible track's idx after this while loop
        if !is_first_track_visible {
            last_visible_track_idx = idx;
        }

        // Only display the track if its acutally visible
        if is_visible {
            is_first_track_visible = true;

            // Get access to the track customizations
            let custom_tracks: &mut HashMap<usize, TrackCustomization> =
                &mut state.write().custom_tracks;

            track_label(
                ui,
                playlist_rect,
                idx,
                &label_customization,
                top,
                bottom,
                custom_tracks,
            );

            let separator_line = track_separator(
                ui,
                playlist_rect,
                normalized_y_offset,
                idx,
                y_coord,
                &label_customization,
                custom_tracks,
            );

            track_lines.push(separator_line);
        }

        // Add the consumed height to the current height
        current_height += label_customization.height;

        // Track indexes too
        idx += 1;
    }

    // We are going to have multiple layers of responses each capturing something different
    // Allocate a response for the entirety of the playlist
    // The main playlist response should capture scrolling input in order to offset the whole grid
    let ui_base = ui.allocate_rect(playlist_rect, Sense::hover());

    // If there is something dragged over the playlist preview the location of the sample
    if let Some(payload) = ui_base.dnd_hover_payload::<DNDSampleInstance>() {
        // Get cursor position
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            // I think this will always get modified so handling this with option may be overkill
            let mut starting_x = 0.0;
            let mut starting_y = 0.0;

            let mut relative_beat_pos = 0;
            let mut relative_track_pos = 0;

            // Find starting beat position on the x axis
            let mut idx = 0;

            while idx < beat_lines.len() - 1 {
                let lhs = beat_lines[idx];
                let rhs = beat_lines[idx + 1];

                idx += 1;

                if cursor.x > lhs[0].x && cursor.x <= rhs[0].x {
                    starting_x = lhs[0].x;
                    relative_beat_pos = idx;

                    break;
                }
            }

            // Find starting beat position on the y axis
            let mut idx = 0;

            while idx < track_lines.len() - 1 {
                let lhs = track_lines[idx];
                let rhs = track_lines[idx + 1];

                idx += 1;

                if cursor.y > lhs[0].y && cursor.y <= rhs[0].y {
                    starting_y = lhs[0].y;
                    relative_track_pos = idx;

                    break;
                }
            }

            let absolute_track_idx = last_visible_track_idx + relative_track_pos;
            let starting_x = starting_x.max(playlist_rect.left() + TRACK_LABEL_WIDTH as f32);
            let starting_y = starting_y.max(playlist_rect.top());

            let track_customization = get_track_customization(state.clone(), absolute_track_idx);

            // Calculate rectangle length
            let bps = state.read().bpm / 60.;

            // This is basically secs / bps * beat_width
            let rectangle_length =
                symphonia::core::units::Time::from_millis(payload.properties.length as i64)
                    .as_secs() as f32
                    / bps
                    * BEAT_WIDTH as f32;

            let rect_points = [
                Pos2::new(starting_x, starting_y),
                Pos2::new(
                    (starting_x + rectangle_length).min(playlist_rect.right()),
                    (starting_y + track_customization.height)
                        .min(track_lines[relative_track_pos][0].y),
                ),
            ];

            // Draw the rectangle indicating how long the sample is
            ui.painter()
                .rect_filled(Rect::from_points(&rect_points), 0., payload.color);
        }
    }

    // Get cursor position (offest)
    let cursor_offset = state.read().cursor_offset;

    // Draw cursor on playlist
    draw_cursor(ui, playlist_rect, grid_offset, cursor_offset);

    // Capture scroll if hovered
    if ui_base.hovered() {
        let scroll_delta = ui.input(|reader| reader.smooth_scroll_delta());
        state.write().grid_offset = grid_offset.add(scroll_delta * 200.).min(Vec2::default());
    }
}

fn get_track_customization(state: Arc<RwLock<PlaylistState>>, idx: usize) -> TrackCustomization {
    

    match state.read().custom_tracks.get(&idx) {
        Some(custom) => custom.clone(),
        None => TrackCustomization::named_default(idx),
    }
}

/// Draws main cursor (Indicates where we are in current playlist)
fn draw_cursor(ui: &mut Ui, playlist_rect: Rect, grid_offset: Vec2, cursor_offset: f32) {
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
        Stroke::new(STROKE_WIDTH, CURSOR_COLOR),
    );
}

/// Draws beat outlines from the left of the playlist to the right with the step of `beat_width`.
/// The function returns the positions of the lines on the screen.
fn beat_outlines(
    ui: &mut Ui,
    playlist_rect: Rect,
    normalized_x_offset: f32,
    beat_width: f32,
) -> Vec<[Pos2; 2]> {
    let mut line_positions = Vec::new();
    let mut x_coord = playlist_rect.left() + normalized_x_offset;
    let max = playlist_rect.right() - normalized_x_offset;

    // Skip the first four spaces because that is allocated to the track label
    x_coord += beat_width * 4.0;

    while x_coord < max {
        let line_pos = [
            Pos2::new(
                (x_coord + normalized_x_offset)
                    .clamp(playlist_rect.left(), playlist_rect.right()),
                playlist_rect.top(),
            ),
            Pos2::new(
                (x_coord + normalized_x_offset)
                    .clamp(playlist_rect.left(), playlist_rect.right()),
                playlist_rect.bottom(),
            ),
        ];
        ui.painter().line(
            line_pos.to_vec(),
            Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
        );

        // Store the line position
        line_positions.push(line_pos);

        x_coord += beat_width;
    }

    line_positions
}

fn track_label<'a>(
    ui: &mut Ui,
    playlist_rect: Rect,
    idx: usize,
    label_customization: &TrackCustomization,
    top: f32,
    bottom: f32,
    custom_tracks: &mut HashMap<usize, TrackCustomization>,
) {
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

    // Detect if it has been right clicked on and store a entry in the customization list.
    if label.secondary_clicked() && !custom_tracks.contains_key(&idx) {
        custom_tracks.insert(idx, TrackCustomization::named_default(idx));
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
            if *customization_state == TrackCustomization::named_default(idx) {
                custom_tracks.remove(&idx);
            }
        }
    }
}

fn track_separator(
    ui: &mut Ui,
    playlist_rect: Rect,
    normalized_y_offset: f32,
    idx: usize,
    y_coord: f32,
    label_customization: &TrackCustomization,
    custom_tracks: &mut HashMap<usize, TrackCustomization>,
) -> [Pos2; 2] {
    // Draw track separator lines
    let separator_points = [
        Pos2::new(
            playlist_rect.left(),
            (y_coord + normalized_y_offset + label_customization.height)
                .clamp(playlist_rect.top(), playlist_rect.bottom()),
        ),
        Pos2::new(
            playlist_rect.right(),
            (y_coord + normalized_y_offset + label_customization.height)
                .clamp(playlist_rect.top(), playlist_rect.bottom()),
        ),
    ];

    ui.painter().line(
        separator_points.to_vec(),
        Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
    );

    // Allocate a response for being able to set the height of the tracks
    let separator = ui.allocate_rect(
        Rect::from_points(&separator_points).expand2(vec2(0., 2.5)),
        Sense::click_and_drag(),
    );

    // Get how much this has been dragged by
    let height_delta = separator.drag_delta().y;
    let pixel_delta = ui.pixels_per_point() * height_delta;

    // Check if a drag has been started
    if separator.drag_started() && !custom_tracks.contains_key(&idx) {
        custom_tracks.insert(idx, TrackCustomization::named_default(idx));
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
            } else {
                customization_state.height = TRACK_HEIGHT;
            }
        } else {
            customization_state.height = customization_state
                .height
                .add(pixel_delta)
                .max(MINIMUM_TRACK_HEIGHT);
        }
    }

    // Indicate that this can be grabbed
    separator.on_hover_cursor(egui::CursorIcon::ResizeVertical);

    separator_points
}
