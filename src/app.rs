const SUPPORTED_TYPES: [&str; 3] = ["wav", "mp3", "flac"];

use eframe::{App, CreationContext};
use egui::{vec2, Align2, Color32, FontId, ScrollArea, Sense, Slider};
use rodio::{OutputStream, OutputStreamHandle, Sink};

use derive_more::derive::Debug;

use std::{io::BufReader, path::PathBuf, usize};

use crate::{MusicGrid, SoundNode};

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct MediaFile {
    path: PathBuf,

    #[serde(skip)]
    #[debug(skip)]
    sink: Option<Sink>,
}

impl MediaFile {
    fn new(path: PathBuf, sink: Option<Sink>) -> Self {
        Self { path, sink }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Application {
    music_grid: MusicGrid,
    media_files: Vec<MediaFile>,
    media_panel_is_open: bool,

    #[debug(skip)]
    #[serde(skip)]
    audio_playback: Option<(OutputStream, OutputStreamHandle)>,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            music_grid: MusicGrid::new(10),
            media_files: vec![],
            media_panel_is_open: false,
            audio_playback: OutputStream::try_default().ok(),
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
        egui_extras::install_image_loaders(ctx);

        egui::TopBottomPanel::top("setts").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Add node").clicked() {
                    self.music_grid
                        .nodes_mut()
                        .insert(3, SoundNode::new("Fasz".to_string(), 200));
                }

                if ui.button("Start").clicked() {
                    let playback_line = self.music_grid.playback_line_mut();

                    playback_line;
                }

                ui.button("Stop").clicked();

                ui.menu_button("Panels", |ui| {
                    if ui.button("Media Panel").clicked() {
                        self.media_panel_is_open = !self.media_panel_is_open;
                    }
                });

                ui.add(Slider::new(
                    self.music_grid.beat_per_minute_mut(),
                    1.0_f64..=495.0_f64,
                ));
            });
        });

        egui::SidePanel::left("media").show_animated(ctx, self.media_panel_is_open, |ui| {
            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for media_file in self.media_files.iter_mut() {
                        ui.horizontal(|ui| {
                            if let Some((_, output_stream_handle)) = &self.audio_playback {
                                let image = ui.allocate_ui(vec2(20., 20.), |ui| {
                                    ui.image(egui::include_image!("..\\assets\\sound_icon.png"));
                                });

                                let image_interaction = ui.interact(
                                    image.response.rect,
                                    ui.unique_id(),
                                    Sense::click_and_drag(),
                                );

                                if image_interaction.clicked() {
                                    if let Some(sink) = &media_file.sink {
                                        sink.clear();
                                    } else {
                                        //Preview the audio, save the sink so that we can use it later
                                        match playback_file(&media_file.path, output_stream_handle)
                                        {
                                            Ok(sink) => {
                                                media_file.sink = Some(sink);
                                            }
                                            Err(err) => {
                                                dbg!(err);
                                            }
                                        }
                                    }
                                }
                            }

                            ui.label(
                                media_file
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                            );

                            ui.menu_button("Settings", |ui| {});
                        });
                    }
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.music_grid.show(ui);

            let hovered_files = ctx.input(|reader| reader.raw.clone().hovered_files);

            if !hovered_files.is_empty() {
                let floating_rect = ui.min_rect().shrink2(vec2(
                    ui.min_rect().width() / 3.,
                    ui.min_rect().height() / 3.,
                ));

                let is_not_supported_file = hovered_files.iter().any(|hovered_file| {
                    !SUPPORTED_TYPES.contains(
                        &hovered_file
                            .path
                            .clone()
                            .unwrap_or_default()
                            .extension()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                            .as_str(),
                    )
                });

                if !is_not_supported_file {
                    ui.painter()
                        .rect_filled(floating_rect, 10., Color32::from_gray(150));

                    ui.painter().text(
                        floating_rect.center(),
                        Align2::CENTER_CENTER,
                        "Add Media files to your Project.",
                        FontId::default(),
                        Color32::BLACK,
                    );
                } else {
                    ui.painter().rect_filled(floating_rect, 10., Color32::RED);

                    ui.painter().text(
                        floating_rect.center(),
                        Align2::CENTER_CENTER,
                        "Unsupported Media File.",
                        FontId::default(),
                        Color32::BLACK,
                    );
                }
            }

            let dropped_files = ctx.input(|reader| reader.raw.clone().dropped_files);

            let are_files_not_supported = dropped_files.iter().any(|hovered_file| {
                !SUPPORTED_TYPES.contains(
                    &hovered_file
                        .path
                        .clone()
                        .unwrap_or_default()
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                        .as_str(),
                )
            });

            if !are_files_not_supported {
                for dropped_file in dropped_files {
                    if let Some(path) = dropped_file.path {
                        self.media_files.push(MediaFile::new(path, None));
                    }
                }
            }
        });
    }
}

pub fn playback_file(path: &PathBuf, stream_handle: &OutputStreamHandle) -> anyhow::Result<Sink> {
    let sink = rodio::Sink::try_new(stream_handle)?;

    let file = std::fs::File::open(path)?;

    sink.append(rodio::Decoder::new(BufReader::new(file))?);

    Ok(sink)
}
