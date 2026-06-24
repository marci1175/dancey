use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use egui::{CentralPanel, Direction, Id, InnerResponse, Ui, Vec2, ViewportBuilder, ViewportId};
use egui_toast::{Toast, ToastOptions, ToastStyle, Toasts};
use indexmap::IndexSet;
use parking_lot::{Mutex, RwLock};
use strum::IntoDiscriminant;

use crate::ui::panels::{
    media::{FileSystemSelector, MediaPanel, mediapicker_ui},
    playlist::{PlaylistState, playlist_ui},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::EnumDiscriminants)]
#[strum_discriminants(derive(Hash))]
/// Enum for labeling all the different panels of the application with their states paired
pub enum PanelId {
    /// Acts as the root for the application, this is the lowest layer of ui.
    Root,

    /// Media selector
    Media(Arc<RwLock<MediaPanel>>),

    /// Playlist
    /// This is where we assemble the music from the clips
    Playlist(Arc<RwLock<PlaylistState>>),

    PluginManager,
    Mixer,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum PanelType {
    Central,
    Left,
    Right,
    Top,
    Bottom,
}

/// A dedicated portion of the ui.
/// All panels are detachable into children uis.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Panel {
    /// Specifies the type of the panel.
    pub id: PanelId,

    /// Tells if the panel is detached from the root ui.
    /// This field is thread safe, because we might need to access this from the child window.
    pub detached: Arc<AtomicBool>,

    /// A separate list of toasts
    /// This is used to display toasts on the child windows
    /// If the panel is attached to the parent window this toast field is not used.
    #[serde(skip)]
    pub toasts: Arc<Mutex<Toasts>>,

    #[serde(skip)]
    /// Viewport settings for the detached window.
    pub viewport_settings: ViewportBuilder,

    pub panel_type: PanelType,
}

impl Panel {
    pub fn display(&self, ui: &mut Ui) {
        match &self.id {
            PanelId::Media(state) => {
                display_panel(self, ui, state.clone(), "Media Picker", mediapicker_ui)
            }
            PanelId::Playlist(state) => {
                display_panel(self, ui, state.clone(), "Playlist", playlist_ui)
            }
            PanelId::PluginManager => todo!(),
            PanelId::Root => todo!(),
            PanelId::Mixer => todo!(),
        };
    }
}

impl Panel {
    pub fn new(id: PanelId, viewport_settings: ViewportBuilder, ty: PanelType) -> Self {
        Self {
            id,
            detached: Arc::new(AtomicBool::new(false)),
            toasts: Arc::new(Mutex::new(Toasts::new().direction(Direction::TopDown))),
            viewport_settings,
            panel_type: ty,
        }
    }
}

/// This function creates the default state of the panels
pub fn create_panels() -> Vec<Panel> {
    vec![
        // Media picker
        Panel::new(
            PanelId::Media(Arc::new(RwLock::new(MediaPanel {
                media_selector_state: crate::ui::panels::media::MediaSelectorState::Bookmarks,
                bookmarks: IndexSet::new(),
                filesystem_selector_state: FileSystemSelector::default(),
            }))),
            ViewportBuilder {
                title: Some(String::from("Media")),
                app_id: None,
                position: None,
                inner_size: None,
                min_inner_size: None,
                max_inner_size: None,
                clamp_size_to_monitor_size: None,
                fullscreen: None,
                maximized: None,
                resizable: Some(true),
                transparent: Some(false),
                decorations: Some(true),
                icon: None,
                active: Some(true),
                visible: Some(true),
                fullsize_content_view: None,
                title_shown: Some(false),
                titlebar_buttons_shown: Some(false),
                titlebar_shown: Some(false),
                drag_and_drop: Some(false),
                taskbar: Some(false),
                close_button: Some(false),
                minimize_button: Some(true),
                maximize_button: Some(true),
                window_level: Some(egui::WindowLevel::Normal),
                mouse_passthrough: None,
                window_type: Some(egui::X11WindowType::Normal),
                movable_by_window_background: None,
                has_shadow: None,
                override_redirect: None,
            },
            PanelType::Left,
        ),
        // Playlist
        Panel::new(
            PanelId::Playlist(Arc::new(RwLock::new(PlaylistState {
                bpm: 120.0,
                cursor_offset: 0.0,
                grid_offset: Vec2::default(),
                custom_tracks: HashMap::new(),
            }))),
            ViewportBuilder {
                title: Some(String::from("Playlist")),
                app_id: None,
                position: None,
                inner_size: None,
                min_inner_size: None,
                max_inner_size: None,
                clamp_size_to_monitor_size: None,
                fullscreen: None,
                maximized: None,
                resizable: Some(true),
                transparent: Some(false),
                decorations: Some(true),
                icon: None,
                active: Some(true),
                visible: Some(true),
                fullsize_content_view: None,
                title_shown: Some(false),
                titlebar_buttons_shown: Some(false),
                titlebar_shown: Some(false),
                drag_and_drop: Some(false),
                taskbar: Some(false),
                close_button: Some(false),
                minimize_button: Some(true),
                maximize_button: Some(true),
                window_level: Some(egui::WindowLevel::AlwaysOnTop),
                mouse_passthrough: None,
                window_type: Some(egui::X11WindowType::Normal),
                movable_by_window_background: None,
                has_shadow: None,
                override_redirect: None,
            },
            PanelType::Central,
        ),
    ]
}

pub fn display_panel_title(this: &Panel, ui: &mut Ui, title: &str) {
    egui::Sides::new().show(
        ui,
        |ui| ui.label(title),
        |ui| {
            if ui
                .button({
                    match this.detached.load(std::sync::atomic::Ordering::Relaxed) {
                        true => "Reattach",
                        false => "Detach",
                    }
                })
                .clicked()
            {
                // Perform a not operation on the current state
                this.detached
                    .fetch_not(std::sync::atomic::Ordering::Relaxed);

                // Repaint root to close window
                ui.ctx().request_repaint_of(ViewportId::ROOT);
            }
        },
    );

    ui.separator();
}

/// For some reason if something is blocking inside the result the toast time get distorted.
/// TODO: Find the issue here
pub fn display_error_as_toast<T, E: ToString>(
    result: Result<T, E>,
    style: ToastStyle,
    toasts: Arc<Mutex<Toasts>>,
) -> Option<T> {
    match result {
        Ok(ret) => Some(ret),
        Err(err) => {
            toasts.lock().add(
                Toast::new()
                    .kind(egui_toast::ToastKind::Error)
                    .text(err.to_string())
                    .style(style)
                    .options(ToastOptions::default().duration(Some(Duration::from_secs(10)))),
            );

            None
        }
    }
}

/// Display a detachable panel in a pre-determined position.
pub fn display_panel<T: Send + Sync + 'static + Clone>(
    this: &Panel,
    ui: &mut Ui,
    state: T,
    title: &'static str,
    display_ui: impl FnOnce(&Panel, &mut Ui, T) + std::marker::Send + std::marker::Sync + 'static + Copy,
) -> Option<InnerResponse<()>> {
    // Allocate the sidepanel for the panel
    // Match the detached panel's state
    match this.detached.load(std::sync::atomic::Ordering::Relaxed) {
        true => {
            // Clone panel state so that it can be cloned into a child window
            let this = this.clone();

            // Create child window for the detached panel
            ui.ctx().show_viewport_deferred(
                ViewportId::from_hash_of(this.id.discriminant()),
                this.viewport_settings.clone(),
                move |ui, _viewport| {
                    // Clone state here to that we can move it
                    let state = state.clone();
                    let toasts = this.toasts.clone();

                    // I am not sure why this is not working when creating the panel and the viewport
                    // If i uncomment this `ctx.request_repaint_of(ViewportId::ROOT);` wont work as intended
                    // ctx.send_viewport_cmd(egui::ViewportCommand::Title(String::from("Media")));

                    CentralPanel::default().show_inside(ui, |ui| {
                        // Display the title of the panel
                        display_panel_title(&this, ui, title);
                        (display_ui)(&this, ui, state);

                        // Display toasts added to child window
                        toasts.lock().show(ui);
                    });
                },
            );

            None
        }
        false => Some({
            // Allocate the area in the root ui based on the type
            match this.panel_type {
                super::lib::PanelType::Central => {
                    egui::CentralPanel::default_margins().show_inside(ui, |ui| {
                        // Display the title of the panel
                        display_panel_title(this, ui, title);

                        // Display ui of the panel
                        (display_ui)(this, ui, state)
                    })
                }
                super::lib::PanelType::Left => {
                    egui::Panel::left(Id::new(this.id.discriminant())).show_inside(ui, |ui| {
                        // Display the title of the panel
                        display_panel_title(this, ui, title);

                        // Display ui of the panel
                        (display_ui)(this, ui, state)
                    })
                }
                super::lib::PanelType::Right => {
                    egui::Panel::right(Id::new(this.id.discriminant())).show_inside(ui, |ui| {
                        // Display the title of the panel
                        display_panel_title(this, ui, title);

                        // Display ui of the panel
                        (display_ui)(this, ui, state)
                    })
                }
                super::lib::PanelType::Top => {
                    egui::Panel::top(Id::new(this.id.discriminant())).show_inside(ui, |ui| {
                        // Display the title of the panel
                        display_panel_title(this, ui, title);

                        // Display ui of the panel
                        (display_ui)(this, ui, state)
                    })
                }
                super::lib::PanelType::Bottom => {
                    egui::Panel::bottom(Id::new(this.id.discriminant())).show_inside(ui, |ui| {
                        // Display the title of the panel
                        display_panel_title(this, ui, title);

                        // Display ui of the panel
                        (display_ui)(this, ui, state)
                    })
                }
            }
        }),
    }
}
