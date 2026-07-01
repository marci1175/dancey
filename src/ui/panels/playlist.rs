use std::{collections::HashMap, ops::Add, path::PathBuf, sync::Arc};

use crate::{
    internals::{
        sample::{SampleProperties, generate_sample_waveform},
        utils::find_value_inbetween,
    },
    ui::panels::{
        lib::{Panel, PanelStates, display_error_as_toast, random_color_with_opacity},
        media::WorkspaceSampleAttributes,
    },
};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, vec2};
use egui_toast::{Toast, ToastStyle};
use indexmap::IndexMap;
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SampleInstance {
    pub name: String,
    pub color: Color32,
    pub path: PathBuf,
    pub properties: SampleProperties,
    pub waveform_map: Option<Vec<[f32; 2]>>,
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

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Position {
    pub track: usize,
    pub beat: usize,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum PlaybackState {
    /// When the plaback is currently ongoing
    Playing,
    /// When the placback has been stopped
    Paused,
    /// When the player hasnt been initalized
    #[default]
    Stopped,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
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

    pub samples: IndexMap<Position, SampleInstance>,

    pub playback_state: PlaybackState,
}

const BPM_PRESETS: &[f32] = &[
    60.0, 70.0, 80.0, 90.0, 100.00, 110.0, 120.0, 128.0, 140.0, 165.0, 174.0,
];

pub fn playlist_ui(_this: &Panel, ui: &mut Ui, global_state: Arc<PanelStates>) {
    let state = &global_state.playlist_panel;

    // Draw the main options / tools for this ui
    ui.horizontal(|ui| {
        let current_playback_state = state.read().playback_state.clone();

        // Display playback main controls based on current state
        match current_playback_state {
            PlaybackState::Playing => {
                if ui.button("Pause").clicked() {
                    state.write().playback_state = PlaybackState::Paused;
                };
            }
            PlaybackState::Paused => {
                if ui.button("Unpause").clicked() {
                    state.write().playback_state = PlaybackState::Playing;
                };
            }
            PlaybackState::Stopped => {
                if ui.button("Play").clicked() {
                    state.write().playback_state = PlaybackState::Paused;
                }
            }
        }

        // Only enable this button if its not stopped
        ui.add_enabled_ui(current_playback_state != PlaybackState::Stopped, |ui| {
            if ui.button("Stop").clicked() {
                state.write().playback_state = PlaybackState::Stopped;
            }
        });

        ui.separator();

        if ui.button("Patterns").clicked() {};

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

    let y_offset_ratio = grid_offset.y / playlist_rect.height();
    let x_offset_ratio = grid_offset.x / playlist_rect.width();

    // Track the positions of the lines drawn so that we can visualize the preview of a sample in the playlist.
    // `first_visible_beat` tells us which absolute beat number `beat_lines[0]` corresponds to,
    // since the vec itself is scroll-relative (index 0 = "first beat currently on screen").
    let (first_visible_beat, beat_lines) =
        beat_outlines(ui, playlist_rect, x_offset_ratio, BEAT_WIDTH as f32);

    // Initalize the track lines list with the topmost line first.
    let mut track_lines = vec![[
        Pos2::new(playlist_rect.left(), playlist_rect.top()),
        Pos2::new(playlist_rect.right(), playlist_rect.top()),
    ]];

    let mut current_height = playlist_rect.top() + y_offset_ratio;

    // The index for tracking the painting of each track
    let mut idx = 0;

    let max_height = playlist_rect.bottom() - y_offset_ratio;

    // Indexes of the tracks which fulfill a certain criterium
    let mut first_visible_track_idx = 0;
    let mut last_visible_track_idx = 0;
    let mut is_first_track_visible = false;

    // Draw track labels (filled rect) and track separator lines
    // This rectangle takes up four bar widths
    while current_height < max_height {
        let y_coord = current_height;

        // Try getting the customization state for the current label
        let label_customization = get_track_customization(state, idx);

        let top = (y_coord + y_offset_ratio).max(playlist_rect.top());
        let bottom =
            (y_coord + y_offset_ratio + label_customization.height).min(playlist_rect.bottom());

        let is_visible = !(top >= playlist_rect.bottom() || bottom <= playlist_rect.top());

        // This will always be set to the last visible track's idx after this while loop
        if !is_first_track_visible {
            first_visible_track_idx = idx;
        }

        // Only display the track if its acutally visible
        // Render currently visible tracks
        if is_visible {
            is_first_track_visible = true;

            // Get access to the track customizations
            let custom_tracks: &mut HashMap<usize, TrackCustomization> =
                &mut state.write().custom_tracks;

            // Draw track labels
            track_label(
                ui,
                playlist_rect,
                idx,
                &label_customization,
                top,
                bottom,
                custom_tracks,
            );

            // Draw separator lines
            let separator_line = track_separator(
                ui,
                playlist_rect,
                y_offset_ratio,
                idx,
                y_coord,
                &label_customization,
                custom_tracks,
            );

            // This will automatically set the index to the last visible track's index
            last_visible_track_idx = idx;

            track_lines.push(separator_line);
        }

        // Add the consumed height to the current height
        current_height += label_customization.height;

        // Track indexes too
        idx += 1;
    }

    // The space after the track labels
    let usable_playlist_rect =
        playlist_rect.with_min_x(playlist_rect.min.x + TRACK_LABEL_WIDTH as f32);

    // Render currently present samples in the playlist
    // We should render the samples because when we are creating them we are also allocation responses
    // These responses would steal the input from the user if created after checking for input over the entire playlist.
    render_samples(
        ui,
        state,
        first_visible_track_idx,
        last_visible_track_idx,
        first_visible_beat,
        usable_playlist_rect,
        &track_lines,
        &beat_lines,
    );

    // We are going to have multiple layers of responses each capturing something different
    // Allocate a response for the entirety of the playlist
    // The main playlist response should capture scrolling input in order to offset the whole grid
    let ui_base = ui.allocate_rect(playlist_rect, Sense::hover());

    // If there is something dragged over the playlist preview the location of the sample
    hover_sample(
        ui,
        state,
        playlist_rect,
        &track_lines,
        &beat_lines,
        first_visible_track_idx,
        &ui_base,
    );

    // Handle the sample if it is dropped into the playlist.
    drop_sample(
        _this,
        ui,
        state,
        global_state.clone(),
        &track_lines,
        &beat_lines,
        first_visible_track_idx,
        first_visible_beat,
        &ui_base,
    );

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

fn render_samples(
    ui: &mut Ui,
    state: &RwLock<PlaylistState>,
    before_first_visible_track_idx: usize,
    last_visible_track_idx: usize,
    first_visible_beat: usize,
    playlist_rect: Rect,
    track_lines: &[[Pos2; 2]],
    beat_lines: &[[Pos2; 2]],
) {
    // Iterate over the samples and decide which one is in frame.
    let samples = state.read().samples.clone();

    for (pos, sample) in samples {
        // Check if the track is visible based on the track
        if !(pos.track >= before_first_visible_track_idx && pos.track <= last_visible_track_idx) {
            continue;
        }

        let line_idx = pos.beat as i64 - first_visible_beat as i64;

        let start_pos = if line_idx >= 0 && (line_idx as usize) < beat_lines.len() {
            beat_lines[line_idx as usize][0].x
        } else if !beat_lines.is_empty() {
            beat_lines[0][0].x + (line_idx as f32) * BEAT_WIDTH as f32
        } else {
            continue;
        };

        // Get track customization
        let _track_customization = get_track_customization(state, pos.track);

        // Calculate rectangle length
        let bps = state.read().bpm / 60.;

        // This is basically secs / bps * beat_width
        let rectangle_length =
            symphonia::core::units::Time::from_millis(sample.properties.length as i64).as_secs()
                as f32
                / bps
                * BEAT_WIDTH as f32;

        // If the sample isn't long enough to reach onto the screen, skip it.
        if start_pos + rectangle_length < playlist_rect.left() {
            continue;
        }

        // Create the rect where the sample might be rendered.
        let sample_rect = Rect::from_min_max(
            Pos2 {
                x: start_pos,
                y: (track_lines[pos.track - before_first_visible_track_idx][0].y),
            },
            Pos2 {
                x: (start_pos + rectangle_length),
                y: (track_lines[pos.track - before_first_visible_track_idx + 1][0].y),
            },
        );

        // Draw sample rect
        ui.painter()
            .with_clip_rect(playlist_rect)
            .rect_filled(sample_rect, 0., sample.color);

        // Create galley for sample label
        let galley = ui.fonts_mut(|f| {
            f.layout(
                sample.name.clone(),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
                sample_rect.width(),
            )
        });

        // Draw sample text
        ui.painter()
            .with_clip_rect(playlist_rect)
            .with_clip_rect(sample_rect)
            .galley(sample_rect.left_top(), galley.clone(), egui::Color32::WHITE);

        // Allocate a response over the sample to capture any inputs it receives
        let sample_response = ui.allocate_rect(sample_rect, Sense::all());

        // Draw the waveform of the sample
        let waveform_rect = sample_rect.shrink2(vec2(0., galley.rect.height()));

        // Only display the waveform if we actually have smth to display
        if let Some(waveform) = &sample.waveform_map {
            // Decide each columns width
            let column_width = waveform_rect.width() / waveform.len() as f32;

            let baseline_maximum_offset = waveform_rect.height() / 2.0;
            let middle_y = waveform_rect.top() + baseline_maximum_offset;

            // Fetch positions over sample
            let start = Pos2::new(waveform_rect.left(), middle_y);
            let end = Pos2::new(waveform_rect.right(), middle_y);

            // Draw a centerline serving as the indication for silence.
            ui.painter()
                .with_clip_rect(playlist_rect)
                .line([start, end].to_vec(), Stroke::new(1.0_f32, Color32::WHITE));

            // Iter over all the samples and draw them
            // We are going to ratio this based on the highest/lowest value the output can get which is 1.0 and -1.0
            // There for the top of this rect is going to serve as 1.0 and the bottom is -1.0
            let mut idx = 0;
            let scale_reference = waveform
                .iter()
                .flat_map(|[min, max]| [min.abs(), max.abs()])
                .fold(0.0_f32, f32::max)
                .max(f32::EPSILON);

            // Draw all of the columns on the screen
            while idx < waveform.len() {
                // The maximum values goes on top of the baseline and the minimum below it
                let [min, max] = waveform[idx];

                // The x coordinate we are operation on
                let x_offset = column_width * idx as f32;

                let x = waveform_rect.left() + x_offset;

                // Starting location of the column
                let baseline = Pos2::new(x, middle_y);

                let normalized_max = max / scale_reference;
                let normalized_min = min / scale_reference;

                // Height of the column we are drawing
                let height_max = -normalized_max * baseline_maximum_offset;
                let height_min = -normalized_min * baseline_maximum_offset;

                // Draw max
                ui.painter().with_clip_rect(playlist_rect).line(
                    [baseline, Pos2::new(x, middle_y + height_max)].to_vec(),
                    Stroke::new(column_width, Color32::WHITE),
                );
                // Draw min
                ui.painter().with_clip_rect(playlist_rect).line(
                    [baseline, Pos2::new(x, middle_y + height_min)].to_vec(),
                    Stroke::new(column_width, Color32::WHITE),
                );

                // Increment index
                idx += 1;
            }
        }

        // If the sample is dragged, simulate a dnd again
        sample_response.dnd_set_drag_payload(sample.clone());

        // Remove the old position of the sample
        if sample_response.drag_stopped() {
            state.write().samples.swap_remove(&pos);
        }
    }
}

fn drop_sample(
    this: &Panel,
    ui: &mut Ui,
    state: &RwLock<PlaylistState>,
    global_state: Arc<PanelStates>,
    track_lines: &[[Pos2; 2]],
    beat_lines: &[[Pos2; 2]],
    first_visible_track_idx: usize,
    first_visible_beat: usize,
    ui_base: &egui::Response,
) {
    if let Some(payload) = ui_base.dnd_release_payload::<SampleInstance>() {
        // Get cursor position
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            // Find starting beat position on the x axis (index into beat_lines)
            let (_, relative_beat_pos) =
                find_value_inbetween(beat_lines.iter().map(|v| v[0].x), cursor.x)
                    .unwrap_or_default();

            // Find starting beat position on the y axis
            let (_, relative_track_pos) =
                find_value_inbetween(track_lines.iter().map(|v| v[0].y), cursor.y)
                    .unwrap_or_default();

            // We have to subtract one from the relative position since the first track's position is out of bounds (its the topmost line of the whole playlist)
            let absolute_track_idx = first_visible_track_idx + relative_track_pos - 1;

            let absolute_beat_pos = relative_beat_pos.max(1) - 1 + first_visible_beat;

            // If anything gets dropped into the "workspace" aka the playlist then add it to the workspace files
            // Look up if we have already stored this one sample
            let query = global_state
                .media_panel
                .read()
                .workspace_selector
                .workspace_samples
                .get(&payload.path)
                .cloned();

            // Check if we already have this sample in the workspace tab
            let sample_instance = if let Some(sample_info) = query {
                global_state
                    .media_panel
                    .write()
                    .workspace_selector
                    .workspace_samples
                    .get(&payload.path);

                // If we do have this sample then insert into playlist accordingly
                SampleInstance {
                    name: sample_info.alias.clone(),
                    color: {
                        // If the color of this sample has been modified, the new color should be displayed when reinserted.
                        if payload.color != sample_info.color {
                            payload.color
                        } else {
                            sample_info.color
                        }
                    },
                    path: payload.path.clone(),
                    properties: payload.properties.clone(),
                    waveform_map: sample_info.waveform_map,
                }
            }
            // Initalize new sample in workspace
            // Generate a new random color for it
            else {
                this.toasts.lock().add(
                    Toast::new()
                        .kind(egui_toast::ToastKind::Info)
                        .text(format!("Imported sample `{}`", payload.name)),
                );

                // Map the waveforms of the sample if it hadnt been inserted yet
                let waveform_map = display_error_as_toast(
                    generate_sample_waveform(&payload.path),
                    ToastStyle::default(),
                    this.toasts.clone(),
                );

                let random_color = random_color_with_opacity(120);

                global_state
                    .media_panel
                    .write()
                    .workspace_selector
                    .workspace_samples
                    .insert(
                        payload.path.clone(),
                        WorkspaceSampleAttributes {
                            alias: payload.name.clone(),

                            // All samples have their color synced by default.
                            is_color_synced: true,
                            color: random_color,
                            waveform_map: waveform_map.clone(),
                        },
                    );

                SampleInstance {
                    name: payload.name.clone(),
                    color: random_color,
                    path: payload.path.clone(),
                    properties: payload.properties.clone(),
                    waveform_map: waveform_map,
                }
            };

            // Store sample in playlist
            state.write().samples.insert(
                Position {
                    track: absolute_track_idx,
                    beat: absolute_beat_pos,
                },
                sample_instance.clone(),
            );
        }
    }
}

fn hover_sample(
    ui: &mut Ui,
    state: &RwLock<PlaylistState>,
    playlist_rect: Rect,
    track_lines: &[[Pos2; 2]],
    beat_lines: &[[Pos2; 2]],
    first_visible_track_idx: usize,
    ui_base: &egui::Response,
) {
    if let Some(payload) = ui_base.dnd_hover_payload::<SampleInstance>() {
        // Get cursor position
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            // Find starting beat position on the x axis
            let (starting_x, _relative_beat_pos) =
                find_value_inbetween(beat_lines.iter().map(|v| v[0].x), cursor.x)
                    .unwrap_or_default();

            // Find starting beat position on the y axis
            let (starting_y, relative_track_pos) =
                find_value_inbetween(track_lines.iter().map(|v| v[0].y), cursor.y)
                    .unwrap_or_default();

            // We have to subtract one from the relative position since the first track's position is out of bounds (its the topmost line of the whole playlist)
            let absolute_track_idx = first_visible_track_idx + relative_track_pos - 1;

            // Clamp both x and y for the preview to draw correctly.
            let starting_x = starting_x.max(playlist_rect.left() + TRACK_LABEL_WIDTH as f32);
            let starting_y = starting_y.max(playlist_rect.top());

            // Fetch track attributes
            let track_customization = get_track_customization(state, absolute_track_idx);

            // Calculate rectangle length
            let bps = state.read().bpm / 60.;

            // This is basically secs / bps * beat_width
            let rectangle_length =
                symphonia::core::units::Time::from_millis(payload.properties.length as i64)
                    .as_secs() as f32
                    / bps
                    * BEAT_WIDTH as f32;

            if relative_track_pos >= track_lines.len() {
                return;
            }

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
}

fn get_track_customization(state: &RwLock<PlaylistState>, idx: usize) -> TrackCustomization {
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
fn beat_outlines(
    ui: &mut Ui,
    playlist_rect: Rect,
    x_offset_ratio: f32,
    beat_width: f32,
) -> (usize, Vec<[Pos2; 2]>) {
    let mut line_positions = Vec::new();

    // The position of "beat 0" (first beat after the label region) with no scroll applied.
    let label_end = playlist_rect.left() + beat_width * 4.0;

    // Shift by the scroll offset to find where beat 0 currently sits on screen.
    let beat_zero_x = label_end + x_offset_ratio;

    // How many whole beats have scrolled past beat 0 (positive = scrolled right).
    let beats_past_zero = ((label_end - beat_zero_x) / beat_width).ceil().max(0.0);

    let first_visible_beat = beats_past_zero as usize;
    let mut x_coord = beat_zero_x + beats_past_zero * beat_width;

    while x_coord <= playlist_rect.right() {
        let line_pos = [
            Pos2::new(x_coord, playlist_rect.top()),
            Pos2::new(x_coord, playlist_rect.bottom()),
        ];
        ui.painter().line(
            line_pos.to_vec(),
            Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
        );

        // Store the line position
        line_positions.push(line_pos);

        x_coord += beat_width;
    }

    (first_visible_beat, line_positions)
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
