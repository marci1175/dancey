// Link the file with the UI and the application's source code.
pub mod app;

use eframe::glow::DEBUG_CALLBACK_FUNCTION;
use egui::{
    ahash::HashSet, epaint::EllipseShape, vec2, Align2, Area, Color32, FontId, Label, Pos2,
    RichText, ScrollArea, Vec2,
};
use parking_lot::Mutex;
use rodio::{Decoder, OutputStreamHandle, Sink, Source};
use symphonia::core::{codecs::{CodecParameters, DecoderOptions}, conv::IntoSample, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint, units::TimeBase};

use std::{
    collections::HashMap,
    fs::{self, File},
    hash::Hash,
    io::{BufReader, Cursor, Seek},
    path::PathBuf,
    sync::{
        atomic::AtomicU64,
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    time::Duration,
    usize,
};

use derive_more::derive::Debug;
use egui::{scroll_area::ScrollAreaOutput, Context, Rect, Response, Sense, Stroke, Ui, UiBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct SoundNode {
    name: String,

    #[serde(skip)]
    #[debug(skip)]
    bytes: Cursor<Vec<u8>>,

    nth_node: i64,
    
    #[serde(skip)]
    track_params: CodecParameters,

    duration: f64,
}

impl SoundNode {
    pub fn new(name: String, position: i64, path: PathBuf) -> anyhow::Result<Self> {
        let (bytes, duration, track_params) = parse_audio_file(path)?;

        Ok(
            Self {
                name,
                bytes,
                nth_node: position,
                track_params,
                duration,
            }
        )
    }

    pub fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn clone_relocate(&self, new_nth_node: i64) -> Self {
        Self { name: self.name.clone(), bytes: self.bytes.clone(), nth_node: new_nth_node, track_params: self.track_params.clone(), duration: self.duration.clone() }
    }
}

fn parse_audio_file(path: PathBuf) -> anyhow::Result<(Cursor<Vec<u8>>, f64, CodecParameters)> {
    let bytes = Cursor::new(fs::read(path)?);

    let mss = MediaSourceStream::new(Box::new(bytes.clone()), Default::default());

    let hint = Hint::new();

    let metadata_opts: MetadataOptions = Default::default();
    let format_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

    let format = probed.format;

    let mut tracks = format.tracks().iter();

    let decoder = symphonia::default::get_codecs();
        
    let track = tracks.nth(0).ok_or_else(|| anyhow::Error::msg("No tracks were present in the input file."))?;

    let decoder_options = DecoderOptions::default();

    let track_params = track.codec_params.clone();

    let duration = if let Some(time_base) = &track_params.time_base {
        let duration = time_base.calc_time(track_params.n_frames.ok_or_else(|| anyhow::Error::msg("No frames were present in the input file."))?);
    
        duration.seconds as f64 + duration.frac 
    } else {
        0.0
    };

    let track_params = decoder.make(&track_params, &decoder_options)?.codec_params().clone();

    Ok((bytes, duration, track_params))
}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ItemGroup<K: Eq + Hash, T> {
    inner: HashMap<K, Vec<T>>,
}

impl<K: Eq + Hash, T> ItemGroup<K, T> {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: K, value: T) {
        if let Some(group) = self.inner.get_mut(&key) {
            group.push(value);
        } else {
            self.inner.insert(key, vec![value]);
        }
    }

    /// If the key does not exist, it will not return any errors.
    pub fn remove(&mut self, key: &K, idx: usize) {
        if let Some(group) = self.inner.get_mut(key) {
            group.swap_remove(idx);
        }
    }

    pub fn get(&self, key: K) -> Option<&Vec<T>> {
        self.inner.get(&key)
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut Vec<T>> {
        self.inner.get_mut(&key)
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

/// This is used to iterate over custom numbers, this was originally made for Floats.
struct CustomRange<T> {
    /// Current value of the range.
    current: T,
    /// The destination value which we are incrementing towards.
    end: T,
    /// The step which we increment [`self.current`] with.
    step: T,
}

impl<T> CustomRange<T> {
    fn new(start: T, end: T, step: T) -> Self {
        Self {
            current: start,
            end,
            step,
        }
    }
}

impl<T: std::ops::AddAssign + PartialOrd + Copy> Iterator for CustomRange<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            None
        } else {
            let result = self.current;
            self.current += self.step;
            Some(result)
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MusicGrid {
    /// Currently unused.
    /// This is used to scale every item of the [`MusicGrid`].
    scale: f64,

    /// This field contains all of the [`SoundNode`]-s.
    /// The key is the track's index, and the value is a list of [`SoundNode`]-s.
    nodes: ItemGroup<usize, SoundNode>,

    /// Track count, this shows the count of track's available and allocated.
    track_count: usize,

    #[serde(skip)]
    #[debug(skip)]
    /// The inner state of the [`MusicGrid`]'s UI.
    inner_state: Option<ScrollAreaOutput<()>>,

    /// The beats per minute counter.
    beat_per_minute: f64,

    #[serde(skip)]
    /// The receiver part of the Drag and Drop requester.
    dnd_receiver: Receiver<SoundNode>,

    #[serde(skip)]
    /// The sender part of the Drag and Drop requester.
    dnd_sender: Sender<SoundNode>,

    /// The [`Rect`] where the [`MusicGrid`] widget is displayed.
    grid_rect: Rect,

    /// The total count of samples provided by the tracks. This is used to allocate a buffer which is used to preview the edited sounds/samples.
    /// This is not recounted automaticly (When a drag and drop is initiated), so it can provide inaccurate values.
    /// [`Self::recount_sample_length`] is available for recounting the samples from the Samples.
    total_samples: u128,
}

impl Default for MusicGrid {
    fn default() -> Self {
        let (dnd_sender, dnd_receiver) = channel();

        Self {
            scale: 1.,
            nodes: ItemGroup::new(),
            track_count: 1,
            inner_state: None,
            beat_per_minute: 100.,
            dnd_receiver,
            dnd_sender,
            grid_rect: Rect::NOTHING,
            total_samples: 0,
        }
    }
}

impl MusicGrid {
    pub fn new(channel_count: usize) -> Self {
        let (dnd_sender, dnd_receiver) = channel();

        Self {
            scale: 1.0,
            nodes: ItemGroup::new(),
            track_count: channel_count,
            inner_state: None,
            beat_per_minute: 100.0,
            dnd_receiver,
            dnd_sender,
            grid_rect: Rect::NOTHING,
            total_samples: 0,
        }
    }

    /// Gets a grid's node width. This is influenced by the area allocated to the [`MusicGrid`].
    pub fn get_grid_node_width(&self) -> f32 {
        self.grid_rect.width() / self.beat_per_minute as f32
    }

    /// Displays the [`MusicGrid`], based on the parameters set by the user. (Or the default values)
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

        self.grid_rect = rect;

        let mut x_offset = 0.;
        let mut y_offset = 0.;

        if let Some(state) = &self.inner_state {
            x_offset = state.state.offset.x;
            y_offset = state.state.offset.y;
        }

        let pos_delta = {
            if let Some(state) = &self.inner_state {
                state.state.offset
            } else {
                Vec2::default()
            }
        };

        ui.allocate_new_ui(
            UiBuilder {
                max_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                let painter = ui.painter();

                let style = ui.ctx().style().clone();

                painter.rect_filled(rect, 3., style.visuals.extreme_bg_color);

                for x_coord in CustomRange::new(
                    ui.min_rect().left(),
                    rect.right() + {
                        if let Some(state) = &self.inner_state {
                            state.state.offset.x
                        } else {
                            0.0
                        }
                    },
                    self.get_grid_node_width(),
                ) {
                    painter.line(
                        vec![
                            Pos2::new(x_coord as f32 - x_offset, rect.top()),
                            Pos2::new(x_coord as f32 - x_offset, rect.bottom()),
                        ],
                        Stroke::new(2., style.visuals.weak_text_color()),
                    );
                }

                let dropped_node = self.dnd_receiver.try_recv().ok();

                for (idx, y_coord) in
                    CustomRange::new(rect.top(), 100. * self.track_count as f32 + 1., 100.)
                        .enumerate()
                {
                    painter.line(
                        vec![
                            Pos2::new(rect.left(), y_coord as f32 - y_offset),
                            Pos2::new(rect.right(), y_coord as f32 - y_offset),
                        ],
                        Stroke::new(2., style.visuals.weak_text_color()),
                    );

                    let rect_rect = Rect::from_min_max(
                        Pos2::new(rect.left() + x_offset, y_coord),
                        Pos2::new(rect.right() + x_offset, y_coord + 100.),
                    );

                    if let Some(node) = &dropped_node {
                        let mouse_pointer =
                            ui.ctx().pointer_hover_pos().unwrap_or_default() + pos_delta;

                        if rect_rect.contains(mouse_pointer) {
                            self.nodes.insert(idx + 1, node.clone());
                        }
                    }
                }

                let width_per_sec = rect.width() / 60.;
                let grid_node_width = self.get_grid_node_width();

                let scroll_state = ScrollArea::both()
                    .auto_shrink([false, false])
                    .drag_to_scroll(false)
                    .show_rows(ui, 100., self.track_count + 1, |ui, row_range| {
                        for row in row_range {
                            if let Some(sound_nodes) = self.nodes.get_mut(row) {
                                for (idx, node) in sound_nodes.clone().iter().enumerate() {
                                    let scaled_width =
                                        node.duration as f32
                                            * width_per_sec;

                                    let nth_node_pos =
                                        rect.left() + (node.nth_node as f32 * grid_node_width);

                                    let scaled_position = nth_node_pos - x_offset;

                                    let audio_node_rect = Rect::from_min_max(
                                        Pos2::new(
                                            scaled_position,
                                            (row * 100) as f32 - y_offset - 70.,
                                        ),
                                        Pos2::new(
                                            scaled_position + scaled_width,
                                            ((row + 1) * 100) as f32 - y_offset - 70.,
                                        ),
                                    );

                                    ui.allocate_new_ui(
                                        UiBuilder {
                                            max_rect: Some(audio_node_rect),
                                            ..Default::default()
                                        },
                                        |ui| {
                                            // The reason I allocate this, is to force allocate the width taken up by the rect, so we can navigate accurately between the nodes.
                                            ui.allocate_space(vec2(ui.available_width(), 1.));

                                            ui.painter().rect_filled(
                                                audio_node_rect,
                                                0.,
                                                Color32::from_gray(100),
                                            );

                                            let label = ui.add(
                                                Label::new(
                                                    RichText::from(node.name.clone())
                                                        .color(Color32::WHITE),
                                                )
                                                .selectable(false)
                                                .sense(Sense::drag()),
                                            );

                                            if label.dragged() {
                                                // We are able to unwrap, but I dont want to panic no matter what.
                                                let pointer_pos = ui
                                                    .ctx()
                                                    .pointer_latest_pos()
                                                    .unwrap_or_default();

                                                egui::Area::new("dropped_sound".into()).show(ui.ctx(), |ui| {
                                                    ui.painter().rect_filled(Rect::from_center_size(pointer_pos, vec2(150., 20.)), 5., Color32::GRAY);
                                                    ui.painter().text(pointer_pos, Align2::CENTER_CENTER, node.name.clone(), FontId::default(), Color32::BLACK);
                                                });
                                            }

                                            if label.drag_stopped() {
                                                let new_node = node.clone_relocate(((ui
                                                    .ctx()
                                                    .pointer_hover_pos()
                                                    .unwrap_or_default()
                                                    .x
                                                    - self.grid_rect.left()
                                                    + pos_delta.x)
                                                    / (self.grid_rect.width()
                                                        / self.beat_per_minute as f32))
                                                    as i64);

                                                self.dnd_sender.send(new_node).unwrap();

                                                //Remove the old node
                                                sound_nodes.swap_remove(idx);
                                            }

                                            label.context_menu(|ui| {
                                                ui.label("Settings");

                                                ui.separator();

                                                if ui.button("Delete").clicked() {
                                                    sound_nodes.swap_remove(idx);

                                                    ui.close_menu();
                                                }

                                                ui.menu_button("Rename", |ui| {
                                                    ui.text_edit_singleline(
                                                        &mut sound_nodes[idx].name,
                                                    );
                                                });
                                            });
                                        },
                                    );
                                }
                            }
                        }
                    });

                self.inner_state = Some(scroll_state);
            },
        );

        response
    }

    /// Mutably gets all of the nodes of the [`MusicGrid`].
    /// See [`ItemGroup`] for more documentation.
    pub fn nodes_mut(&mut self) -> &mut ItemGroup<usize, SoundNode> {
        &mut self.nodes
    }

    /// DEPRECATED, CURRENTLY UNUSED
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    /// Mutably gets the beat_per_minute field of [`MusicGrid`].
    /// If this value is modified the grid will automaticly adjust. (This includes adjusting the [`SoundNode`]-s too.)
    pub fn beat_per_minute_mut(&mut self) -> &mut f64 {
        &mut self.beat_per_minute
    }

    /// This function is available for debug uses. If the `self.total_samples` gets reset manually, the value wont update itself.
    /// This function recounts the total number of samples, from the list of the [`CodecParamater`]-s.
    /// This is not that slow, but it gets slower with more samples.
    /// Big O notation: O(n)
    pub fn recount_sample_length(samples: Vec<CodecParameters>) -> anyhow::Result<u128> {
        let mut sample_count = 0;

        // Iter over all of the samples
        for sample in &samples {
            // Increment sample count
            sample_count += (sample.n_frames.ok_or_else(|| anyhow::Error::msg("Input did not contain the n_frames attribute."))? * sample.channels.ok_or_else(|| anyhow::Error::msg("Input did not contain the channels attribute."))?.count() as u64) as u128;
        }

        Ok(sample_count)
    }

    /// This function registers a Drag and Drop request.
    /// It automaticly calculates the position of the node added. (From cursor_pos)
    pub fn regsiter_dnd_drop(
        &mut self,
        // Used to create a `SoundNode` instance, which is added to the `MusicGrid` 
        file_name: String,
        // Used to create a `SoundNode` instance, which is added to the `MusicGrid` 
        path: PathBuf,
        // This is used to calculate the position of the node.
        cursor_pos: Pos2,
    ) -> anyhow::Result<()> {
        // Fetch the offset on the X coordinate.
        let x_pos_offset = if let Some(state) = &self.inner_state {
            state.state.offset.x
        } else {
            0.0
        };

        // Create a new node
        let node = SoundNode::new(
            file_name,
            ((cursor_pos.x - self.grid_rect.left() + x_pos_offset) / self.get_grid_node_width())
                as i64,
            path,
        )?;

        // We should first send the node, and only then increment the inner counter.
        self.dnd_sender.send(node.clone())?;

        // Make sure that when debugging or modify the field we re-measure sample_length, as this implemention is fast but is prone to errors. (Like if the value is manually reset to 0.)
        // A function which will recount this is available: ``
        self.total_samples += (node.track_params.n_frames.ok_or_else(|| anyhow::Error::msg("Input did not contain the n_frames attribute."))? * node.track_params.channels.ok_or_else(|| anyhow::Error::msg("Input did not contain the channels attribute."))?.count() as u64) as u128;

        Ok(())
    }

    pub fn regsiter_dnd_drop_from_node(
        &self,
        node: SoundNode,
    ) -> Result<(), std::sync::mpsc::SendError<SoundNode>> {
        self.dnd_sender.send(node)
    }

    pub fn grid_rect(&self) -> Rect {
        self.grid_rect
    }
}

pub fn playback_file(stream_handle: &OutputStreamHandle, path: PathBuf) -> anyhow::Result<Sink> {
    let source = get_source_from_path(&path)?;

    let sink = create_playbacker(stream_handle, source)?;

    Ok(sink)
}

pub fn create_playbacker(
    stream_handle: &OutputStreamHandle,
    source: Decoder<BufReader<File>>,
) -> anyhow::Result<Sink> {
    let sink = rodio::Sink::try_new(stream_handle)?;

    sink.append(source);

    Ok(sink)
}

pub fn get_source_from_path(
    path: &PathBuf,
) -> Result<rodio::Decoder<BufReader<std::fs::File>>, anyhow::Error> {
    let file = std::fs::File::open(path)?;

    let source = rodio::Decoder::new(BufReader::new(file))?;

    Ok(source)
}
