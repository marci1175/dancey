use std::{
    collections::HashMap,
    hash::Hash,
    sync::{atomic::AtomicU64, Arc},
};

use derive_more::derive::Debug;
use egui::{
    scroll_area::ScrollAreaOutput, Color32, Context, Pos2, Rect, Response, ScrollArea, Sense,
    Stroke, Ui, UiBuilder,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SoundNode {
    name: String,
    samples: (),
    position: i64,
}

impl SoundNode {
    pub fn new(name: String, position: i64) -> Self {
        Self {
            name,
            samples: (),
            position,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub fn name(&self) -> &str {
        &self.name
    }
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

    pub fn get(&self, key: K) -> Option<&Vec<T>> {
        self.inner.get(&key)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MusicGrid {
    scale: f64,

    nodes: ItemGroup<usize, SoundNode>,

    playback_line: PlaybackLine,

    channel_count: usize,

    #[serde(skip)]
    #[debug(skip)]
    inner_state: Option<ScrollAreaOutput<()>>,

    beat_per_minute: f64,
}

impl MusicGrid {
    pub fn new(channel_count: usize) -> Self {
        Self {
            scale: 1.0,
            nodes: ItemGroup::new(),
            playback_line: PlaybackLine::default(),
            channel_count,
            inner_state: None,
            beat_per_minute: 100.0,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

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

                for x_coord in
                    (0..(rect.width()) as i32).step_by((500.0 - self.beat_per_minute) as usize)
                {
                    painter.line(
                        vec![
                            Pos2::new(x_coord as f32 - x_offset, rect.top()),
                            Pos2::new(x_coord as f32 - x_offset, rect.bottom()),
                        ],
                        Stroke::new(2., style.visuals.weak_text_color()),
                    );
                }

                for y_coord in (0..100 * self.channel_count + 1).step_by(100) {
                    painter.line(
                        vec![
                            Pos2::new(rect.left(), y_coord as f32 - y_offset),
                            Pos2::new(rect.right(), y_coord as f32 - y_offset),
                        ],
                        Stroke::new(2., style.visuals.weak_text_color()),
                    );
                }

                let scroll_state = ScrollArea::both().auto_shrink([false, false]).show_rows(
                    ui,
                    100.,
                    self.channel_count + 1,
                    |ui, row_range| {
                        for row in row_range {
                            if let Some(sound_nodes) = self.nodes.get(row) {
                                for node in sound_nodes {
                                    let response = ui.allocate_rect(
                                        Rect::from_min_max(
                                            Pos2::new(
                                                node.position as f32,
                                                (row * 100) as f32 - y_offset,
                                            ),
                                            Pos2::new(
                                                (node.position + 50) as f32,
                                                ((row + 1) * 100) as f32 - y_offset,
                                            ),
                                        ),
                                        Sense::click(),
                                    );

                                    ui.painter().rect_filled(
                                        Rect::from_min_max(
                                            Pos2::new(
                                                node.position as f32,
                                                (row * 100) as f32 - y_offset,
                                            ),
                                            Pos2::new(
                                                (node.position + 50) as f32,
                                                ((row + 1) * 100) as f32 - y_offset,
                                            ),
                                        ),
                                        0.,
                                        Color32::GREEN,
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
}
