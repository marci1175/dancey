// Link the file with the UI and the application's source code.
pub mod app;

use egui::{
    ahash::HashSet, epaint::EllipseShape, vec2, Area, Color32, Pos2, RichText, ScrollArea, Vec2,
};
use rodio::{Decoder, Source};

use std::{
    collections::HashMap,
    fs::File,
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
    samples: Option<Arc<Decoder<Cursor<Vec<u8>>>>>,

    nth_node: i64,
    position_reference_width: f32,

    duration: Option<Duration>,
}

impl SoundNode {
    pub fn new(name: String, position: i64, path: PathBuf, position_reference_width: f32) -> Self {
        let mut duration = None;
        let samples = if let Ok(decoder) = create_decoder(path) {
            duration = decoder.total_duration();

            Some(Arc::new(decoder))
        } else {
            None
        };

        Self {
            name,
            samples,
            nth_node: position,
            position_reference_width,
            duration,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

fn create_decoder(path: PathBuf) -> anyhow::Result<Decoder<Cursor<Vec<u8>>>> {
    let file_content: Vec<u8> = std::fs::read(path)?; // Extract the Vec<u8> from the Result

    let cursor = Cursor::new(file_content); // Wrap Vec<u8> in a Cursor

    let decoder = Decoder::new(cursor)?; // Create the Decoder

    Ok(decoder)
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PlaybackLine {
    pos: Arc<AtomicU64>,
}

impl PlaybackLine {
    fn start(&mut self, ctx: &Context) {}
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

struct FloatRange<T> {
    current: T,
    end: T,
    step: T,
}

impl<T> FloatRange<T> {
    fn new(start: T, end: T, step: T) -> Self {
        Self {
            current: start,
            end,
            step,
        }
    }
}

impl<T: std::ops::AddAssign + PartialOrd + Copy> Iterator for FloatRange<T> {
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
    scale: f64,

    nodes: ItemGroup<usize, SoundNode>,

    playback_line: PlaybackLine,

    channel_count: usize,

    #[serde(skip)]
    #[debug(skip)]
    inner_state: Option<ScrollAreaOutput<()>>,

    beat_per_minute: f64,

    #[serde(skip)]
    dnd_receiver: Receiver<SoundNode>,

    #[serde(skip)]
    dnd_sender: Sender<SoundNode>,

    grid_rect: Rect,
}

impl Default for MusicGrid {
    fn default() -> Self {
        let (dnd_sender, dnd_receiver) = channel();

        Self {
            scale: 1.,
            nodes: ItemGroup::new(),
            playback_line: PlaybackLine::default(),
            channel_count: 1,
            inner_state: None,
            beat_per_minute: 100.,
            dnd_receiver,
            dnd_sender,
            grid_rect: Rect::NOTHING,
        }
    }
}

impl MusicGrid {
    pub fn new(channel_count: usize) -> Self {
        let (dnd_sender, dnd_receiver) = channel();

        Self {
            scale: 1.0,
            nodes: ItemGroup::new(),
            playback_line: PlaybackLine::default(),
            channel_count,
            inner_state: None,
            beat_per_minute: 100.0,
            dnd_receiver,
            dnd_sender,
            grid_rect: Rect::NOTHING,
        }
    }

    pub fn get_grid_node_width(&self) -> f32 {
        self.grid_rect.width() / self.beat_per_minute as f32
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

        self.grid_rect = rect;

        let mut x_offset = 0.;
        let mut y_offset = 0.;

        if let Some(state) = &self.inner_state {
            x_offset = state.state.offset.x;
            y_offset = state.state.offset.y;
        }

        ui.allocate_new_ui(
            UiBuilder {
                max_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                let painter = ui.painter();

                let style = ui.ctx().style().clone();

                painter.rect_filled(rect, 3., style.visuals.extreme_bg_color);

                for x_coord in FloatRange::new(
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
                    FloatRange::new(rect.top(), 100. * self.channel_count as f32 + 1., 100.)
                        .enumerate()
                {
                    painter.line(
                        vec![
                            Pos2::new(rect.left(), y_coord as f32 - y_offset),
                            Pos2::new(rect.right(), y_coord as f32 - y_offset),
                        ],
                        Stroke::new(2., style.visuals.weak_text_color()),
                    );

                    let channel_rect = Rect::from_min_max(
                        Pos2::new(rect.left(), y_coord),
                        Pos2::new(rect.right(), y_coord + 100.),
                    );

                    if let Some(node) = &dropped_node {
                        let pos_delta = {
                            if let Some(state) = &self.inner_state {
                                state.state.offset
                            } else {
                                Vec2::default()
                            }
                        };

                        if channel_rect
                            .contains(ui.ctx().pointer_hover_pos().unwrap_or_default() + pos_delta)
                        {
                            self.nodes.insert(idx + 1, node.clone());
                        }
                    }
                }

                let width_per_sec = rect.width() / 60.;
                let grid_node_width = self.get_grid_node_width();

                let scroll_state = ScrollArea::both().drag_to_scroll(false).show_rows(
                    ui,
                    100.,
                    self.channel_count + 1,
                    |ui, row_range| {
                        for row in row_range {
                            if let Some(sound_nodes) = self.nodes.get_mut(row) {
                                for (idx, node) in sound_nodes.clone().iter().enumerate() {
                                    let scaled_width =
                                        node.duration.unwrap_or_default().as_secs_f32()
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

                                    dbg!(audio_node_rect.right());

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

                                            ui.label(
                                                RichText::from(node.name.clone())
                                                    .color(Color32::WHITE),
                                            )
                                            .context_menu(|ui| {
                                                ui.label("Settings");

                                                ui.separator();

                                                if ui.button("Delete").clicked() {
                                                    sound_nodes.swap_remove(idx);

                                                    ui.close_menu();
                                                }
                                            });
                                        },
                                    );
                                }
                            }
                        }
                    },
                );

                self.inner_state = Some(scroll_state);
            },
        );

        response
    }

    pub fn nodes_mut(&mut self) -> &mut ItemGroup<usize, SoundNode> {
        &mut self.nodes
    }

    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    pub fn playback_line_mut(&mut self) -> &mut PlaybackLine {
        &mut self.playback_line
    }

    pub fn beat_per_minute_mut(&mut self) -> &mut f64 {
        &mut self.beat_per_minute
    }

    pub fn regsiter_dnd_drop(
        &mut self,
        file_name: String,
        path: PathBuf,
        cursor_pos: Pos2,
    ) -> Result<(), std::sync::mpsc::SendError<SoundNode>> {
        let x_pos_offset = if let Some(state) = &self.inner_state {
            state.state.offset.x
        } else {
            0.0
        };

        let node = SoundNode::new(
            file_name,
            ((cursor_pos.x - self.grid_rect.left() + x_pos_offset) / self.get_grid_node_width())
                as i64,
            path,
            self.grid_rect().width(),
        );
        self.dnd_sender.send(node)
    }

    pub fn grid_rect(&self) -> Rect {
        self.grid_rect
    }
}
