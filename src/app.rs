const SUPPORTED_TYPES: [&str; 3] = ["wav", "mp3", "flac"];

use eframe::{App, CreationContext};
use egui::{
    vec2, Align2, Color32, FontId, ImageButton, Label, Rect, Response, ScrollArea, Sense, Slider,
};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

use derive_more::derive::Debug;

use std::{fs::File, io::BufReader, path::PathBuf, usize};

use crate::{MusicGrid, SoundNode};

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct MediaFile {
    path: PathBuf,

    #[serde(skip)]
    #[debug(skip)]
    sink: Option<Sink>,
}

impl MediaFile {
    pub fn new(path: PathBuf, sink: Option<Sink>) -> Self {
        Self { path, sink }
    }

    pub fn from_path(path: PathBuf) -> Self {
        Self { path, sink: None }
    }

    pub fn clone_path(&self) -> Self {
        Self {
            path: self.path.clone(),
            sink: None,
        }
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

    #[serde(skip)]
    dragged_media: Option<MediaFile>,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            music_grid: MusicGrid::new(10),
            media_files: vec![],
            media_panel_is_open: false,
            audio_playback: OutputStream::try_default().ok(),

            dragged_media: None,
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
                let music_grid_width = self.music_grid.grid_rect().width();
                if ui.button("Add node").clicked() {
                    self.music_grid.nodes_mut().insert(
                        3,
                        SoundNode::new("Fasz".to_string(), 200, PathBuf::new(), music_grid_width),
                    );
                }

                if ui.button("Start").clicked() {
                    let playback_line = self.music_grid.playback_line_mut();

                    // playback_line;
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

                if ui.button("Clear MusicGrid").clicked() {
                    self.music_grid.nodes.clear();
                }
            });
        });

        egui::SidePanel::left("media").show_animated(ctx, self.media_panel_is_open, |ui| {
            ui.add_space(2.);
            
            ui.horizontal(|ui| {
                if ui.button("Add Media").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Supported audio files", &SUPPORTED_TYPES).pick_file() {
                        self.media_files.push(MediaFile::from_path(path));
                    }
                };

                ui.menu_button("Help", |ui| {
                    ui.label("Information");
                    
                    ui.separator();

                    ui.label("How to use the audio quick preview button:");
                    ui.label("If there is no audio playing, or it has finished playing the left click will automaticly start playing it again.");
                    ui.label("If there is already music player a left-click will pause / unpuase it.");
                    ui.label("The state of the player can be reseted with a right click, which will also stop the music from playing.");
                });
            });

            ui.separator();

            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (_idx, media_file) in self.media_files.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if let Some((_, output_stream_handle)) = &self.audio_playback {
                                    ui.allocate_ui(vec2(20., 20.), |ui| {
                                        let image_icon = ui.add(ImageButton::new(egui::include_image!("..\\assets\\sound_icon.png")).tint({
                                            if let Some(sink) = &media_file.sink {
                                                if sink.is_paused() {
                                                    Color32::RED
                                                }
                                                else if sink.empty() {
                                                    Color32::WHITE
                                                }
                                                else {
                                                    Color32::GREEN
                                                }
                                            }
                                            else {
                                                Color32::WHITE
                                            }
                                        }));

                                        // If the play button is pressed
                                        if image_icon.clicked() {
                                            // If the sink exists check if its paused
                                            if let Some(sink) = &media_file.sink {
                                                // If paused play
                                                if sink.is_paused() {
                                                    sink.play();
                                                }
                                                // If playing pause
                                                else {
                                                    sink.pause();
                                                }
                                            }

                                            // If the media sink doesnt exist create one.
                                            // If the sink has finished playing and the play is pressed again, playback the audio and pause it or anything.
                                            if media_file.sink.is_none() || media_file.sink.as_ref().is_some_and(|sink| sink.empty()) {
                                                //Preview the audio, save the sink so that we can use it later
                                                match playback_file(output_stream_handle, media_file.path.clone())
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

                                        if image_icon.secondary_clicked() {
                                            media_file.sink = None;
                                        }
                                    });

                                    ctx.request_repaint();
                                }

                                let file_name = media_file
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();

                                let label = ui.add(Label::new(file_name.clone()).selectable(false));

                                let interact = label.interact(Sense::click_and_drag());
                                
                                if interact.drag_started() {
                                    self.dragged_media = Some(media_file.clone_path());
                                }

                                if interact.dragged() {
                                    // We are able to unwrap, but I dont want to panic no matter what.
                                    let pointer_pos = ctx.pointer_latest_pos().unwrap_or_default();

                                    egui::Area::new("dropped_sound".into()).show(ctx, |ui| {
                                        ui.painter().rect_filled(Rect::from_center_size(pointer_pos, vec2(150., 20.)), 5., Color32::GRAY);
                                        ui.painter().text(pointer_pos, Align2::CENTER_CENTER, file_name.chars().take(20).collect::<String>(), FontId::default(), Color32::BLACK);
                                    });
                                }

                                if interact.drag_stopped() {
                                    let cursor_pos = ctx.pointer_hover_pos().unwrap_or_default();
                                    
                                    if let Err(err) = self.music_grid.regsiter_dnd_drop(file_name.clone(), media_file.path.clone(), ctx.pointer_hover_pos().unwrap_or_default()) {
                                        dbg!(err);
                                    }

                                    self.dragged_media = None;
                                }
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
                        self.media_files.push(MediaFile::from_path(path));
                    }
                }
            }
        });
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
