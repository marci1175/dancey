const SUPPORTED_TYPES: [&str; 3] = ["wav", "mp3", "flac"];

use eframe::{App, CreationContext};
use egui::{
    vec2, Align2, Color32, ComboBox, FontId, ImageButton, Label, Pos2, Rect, RichText, ScrollArea,
    Sense, Slider, Stroke,
};
use egui_toast::{Toast, Toasts};
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};

use derive_more::derive::Debug;
use tokio::{
    select,
    sync::mpsc::{channel, Sender},
};

use std::{
    ops::{Deref, DerefMut}, path::PathBuf, sync::{atomic::AtomicUsize, Arc}, time::{Duration, Instant}, usize
};

use crate::{playback_file, MusicGrid, PlaybackControl, PlaybackTimer, Settings};

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
    audio_playback: Option<Arc<(OutputStream, OutputStreamHandle)>>,

    #[debug(skip)]
    #[serde(skip)]
    playback_thread_sender: Option<Sender<PlaybackControl>>,

    #[serde(skip)]
    playback_idx: Arc<AtomicUsize>,

    #[debug(skip)]
    #[serde(skip)]
    master_audio_sink: Option<Arc<Sink>>,

    #[serde(skip)]
    dragged_media: Option<MediaFile>,

    #[serde(skip)]
    playback_timer: Option<PlaybackTimer>,

    #[debug(skip)]
    #[serde(skip)]
    toasts: Toasts,

    settings: Settings,
}

impl Default for Application {
    fn default() -> Self {
        let audio_playback: Option<Arc<(OutputStream, OutputStreamHandle)>> =
            OutputStream::try_default().map(Arc::new).ok();
        Self {
            music_grid: MusicGrid::new(10, audio_playback.clone()),
            playback_idx: Arc::new(AtomicUsize::new(0)),
            media_files: vec![],
            media_panel_is_open: false,
            master_audio_sink: None,
            playback_timer: None,
            audio_playback,
            toasts: Toasts::new(),
            dragged_media: None,
            settings: Settings::default(),
            playback_thread_sender: None,
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
                ui.menu_button("Settings", |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.label(RichText::from("Audio").strong());

                        ui.label("Master Volume");

                        let mut current_value = self
                            .settings
                            .master_audio_percent
                            .load(std::sync::atomic::Ordering::Relaxed);

                        ui.add(Slider::new(&mut current_value, 0..=255).suffix("%"));

                        self.settings
                            .master_audio_percent
                            .store(current_value, std::sync::atomic::Ordering::Relaxed);

                        ui.label("Sample Rate");

                        ComboBox::new("sample_rate", "Hz")
                            .selected_text((self.music_grid.sample_rate as usize).to_string())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.music_grid.sample_rate,
                                    crate::SampleRate::ULow,
                                    (crate::SampleRate::ULow as usize).to_string(),
                                );
                                ui.selectable_value(
                                    &mut self.music_grid.sample_rate,
                                    crate::SampleRate::Low,
                                    (crate::SampleRate::Low as usize).to_string(),
                                );
                                ui.selectable_value(
                                    &mut self.music_grid.sample_rate,
                                    crate::SampleRate::Medium,
                                    (crate::SampleRate::Medium as usize).to_string(),
                                );
                                ui.selectable_value(
                                    &mut self.music_grid.sample_rate,
                                    crate::SampleRate::High,
                                    (crate::SampleRate::High as usize).to_string(),
                                );
                                ui.selectable_value(
                                    &mut self.music_grid.sample_rate,
                                    crate::SampleRate::Ultra,
                                    (crate::SampleRate::Ultra as usize).to_string(),
                                );
                            });

                        ui.separator();

                        ui.label(RichText::from("Playback").strong());

                        ui.label("Master Preview Calculation");

                        ComboBox::new("master_preview_calc_select", "Type")
                            .selected_text(match self.settings.master_sample_playback_type {
                                crate::PlaybackImplementation::Simd => "SIMD",
                                crate::PlaybackImplementation::NonSimd => "Non-SIMD",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.master_sample_playback_type,
                                    crate::PlaybackImplementation::Simd,
                                    "SIMD",
                                );
                                ui.selectable_value(
                                    &mut self.settings.master_sample_playback_type,
                                    crate::PlaybackImplementation::NonSimd,
                                    "Non-SIMD",
                                );
                            });

                        ui.separator();
                    });
                });

                ui.menu_button("Panels", |ui| {
                    if ui.button("Media Panel").clicked() {
                        self.media_panel_is_open = !self.media_panel_is_open;
                    }
                });

                ui.add(Slider::new(self.music_grid.beat_per_minute_mut(), 1..=495));

                if ui.button("Clear MusicGrid").clicked() {
                    self.music_grid.nodes.clear();
                }

                ui.add_enabled_ui(self.music_grid.last_node.is_some(), |ui| {
                    if let Some(sink) = &self.master_audio_sink {
                        if ui
                            .button(match sink.is_paused() {
                                true => "Unpause",
                                false => "Pause",
                            })
                            .clicked()
                        {
                            if let Some(sender) = &self.playback_thread_sender {
                                if let Err(err) = sender.try_send(PlaybackControl::Pause) {
                                    dbg!(err.to_string());
                                }
                            }

                            if sink.is_paused() {
                                sink.play();

                                if let Some(timer) = &mut self.playback_timer {
                                    timer.paused_time += timer.pause_started.unwrap().elapsed();

                                    timer.pause_started = None;
                                }
                            } else {
                                sink.pause();
                                
                                if let Some(timer) = &mut self.playback_timer {
                                    timer.pause_started = Some(Instant::now());
                                }
                            }
                        }
                    } else if ui.button("Play").clicked() {
                        let sink = Arc::new(Sink::try_new(&self.audio_playback.as_ref().unwrap().1).unwrap());
                        
                        self.playback_timer = Some(PlaybackTimer::default());
                        self.playback_idx.store(0, std::sync::atomic::Ordering::Relaxed);
                        let playback_idx = self.playback_idx.clone();
                        let sample_rate = self.music_grid.sample_rate as usize;
                        let nodes = self.music_grid.nodes.clone();
                        let beat_per_minute = self.music_grid.beat_per_minute;
                        let sink_clone = sink.clone();

                        let (sender, mut receiver) = channel::<PlaybackControl>(200);

                        self.playback_thread_sender = Some(sender);

                        // Dont change this unless youve chnaged the value in buffer_preview_samples_simd
                        let sample_length_secs = 3;

                        tokio::spawn(async move {
                            let starting_idx = playback_idx.fetch_add(sample_rate * sample_length_secs * 2, std::sync::atomic::Ordering::Relaxed);
                            let dest_idx = playback_idx.load(std::sync::atomic::Ordering::Relaxed);

                            let samples = MusicGrid::buffer_preview_samples_simd(starting_idx, dest_idx, sample_rate, beat_per_minute as usize, &nodes);
                    
                            sink_clone.append(SamplesBuffer::new(
                                2,
                                sample_rate as u32,
                                samples,
                            ));

                            let mut should_playback = true;

                            loop {
                                select! {
                                    _ = tokio::time::sleep(Duration::from_secs(sample_length_secs as u64)) => {
                                        if should_playback {
                                            let starting_idx = playback_idx.fetch_add(sample_rate * sample_length_secs * 2, std::sync::atomic::Ordering::Relaxed);
                                            let dest_idx = playback_idx.load(std::sync::atomic::Ordering::Relaxed);

                                            let samples = MusicGrid::buffer_preview_samples_simd(starting_idx, dest_idx, sample_rate, beat_per_minute as usize, &nodes);
                        
                                            sink_clone.append(SamplesBuffer::new(
                                                2,
                                                sample_rate as u32,
                                                samples,
                                            ));
                                        }
                                    },

                                    Some(seek_control) = receiver.recv() => {
                                        match seek_control {
                                            PlaybackControl::Pause => {
                                                should_playback = !should_playback;
                                            },
                                            PlaybackControl::Stop => {
                                                return;
                                            },
                                            PlaybackControl::Seek(seek_pos) => {
                                                playback_idx.store(seek_pos, std::sync::atomic::Ordering::Relaxed);
                                            },
                                        }
                                    }
                                }
                            }
                        });

                        self.master_audio_sink = Some(sink);
                    }
                });

                ui.add_enabled_ui(self.master_audio_sink.is_some(), |ui| {
                    if ui.button("Stop").clicked() {
                        if let Some(sender) = &self.playback_thread_sender {
                            if let Err(err) = sender.try_send(PlaybackControl::Stop) {
                                dbg!(err.to_string());
                            }

                            self.master_audio_sink.as_ref().unwrap().clear();

                            self.master_audio_sink = None;

                            self.playback_timer = None;
                        }
                    }
                });

                ui.label(format!("Elapsed: {}s", if let Some(timer) = &self.playback_timer {
                    let elapsed_paused = timer.pause_started.map(|instant| { instant.elapsed() }).unwrap_or(Duration::default());

                    let time_playing = timer.playback_started.elapsed() - elapsed_paused - timer.paused_time;
                    
                    time_playing.as_secs_f32()
                } else {
                    0.0
                }));

                if let Some(sink) = &self.master_audio_sink {
                    sink.set_volume(
                        self.settings
                            .master_audio_percent
                            .load(std::sync::atomic::Ordering::Relaxed)
                            as f32
                            / 100.,
                    );
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
                    for media_file in self.media_files.iter_mut() {
                            ui.horizontal(|ui| {
                                if let Some((_, output_stream_handle)) = self.audio_playback.as_deref() {
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
                                        
                                        // Set the sink's volume every frame
                                        if let Some(sink) = &media_file.sink {
                                            // Set the volume of the sink we are currently iterating over
                                            sink.set_volume(1. * (self.settings.master_audio_percent.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.));
                                        }
                                        
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
                                                        self.toasts.add(Toast::new().kind(egui_toast::ToastKind::Error).text(err.to_string()));
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
                                    if let Err(err) = self.music_grid.regsiter_dnd_drop(file_name.clone(), media_file.path.clone(), ctx.pointer_hover_pos().unwrap_or_default()) {
                                        self.toasts.add(Toast::new().kind(egui_toast::ToastKind::Error).text(err.to_string()));
                                    }

                                    self.dragged_media = None;
                                }
                            });
                        }
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.music_grid.show(ui);

            if let Some(sink) = &self.master_audio_sink {
                let beat_dur = 60. / self.music_grid.beat_per_minute as f32
                    * (self.music_grid.beat_per_minute as f32 / 100.);

                if let Some(playback_timer) = &self.playback_timer {
                    let mut elapsed_since_start = playback_timer.playback_started.elapsed();

                    if let Some(pause_started) = playback_timer.pause_started {
                        elapsed_since_start -= (pause_started.elapsed() + playback_timer.paused_time);
                    }
                    else {
                        elapsed_since_start -= playback_timer.paused_time;
                    }


                    let secs_elapsed = elapsed_since_start.as_secs_f32();

                    let x = self.music_grid.grid_rect.left()
                        + (secs_elapsed as f32 / beat_dur) * self.music_grid.get_grid_node_width();

                    let delta_pos = if let Some(state) = &self.music_grid.inner_state {
                        state.state.offset
                    } else {
                        vec2(0., 0.)
                    };

                    ui.painter().line(
                        vec![
                            Pos2::new(x - delta_pos.x, self.music_grid.grid_rect.top()),
                            Pos2::new(x - delta_pos.x, self.music_grid.grid_rect.bottom()),
                        ],
                        Stroke::new(2., Color32::WHITE),
                    );
                }
            }

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
