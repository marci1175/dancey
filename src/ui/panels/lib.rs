use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use egui::{Direction, Ui, ViewportBuilder, ViewportId};
use egui_toast::{Toast, ToastOptions, ToastStyle, Toasts};
use indexmap::IndexSet;
use parking_lot::{Mutex, RwLock};

use crate::ui::panels::{media::{FileSystemSelector, MediaPanel, display_panel, mediapicker_ui}, playlist::playlist_ui};

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
    Playlist
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
}

impl Panel {
    pub fn display(&self, ui: &mut Ui) {
        match &self.id {
            PanelId::Root => todo!(),
            PanelId::Media(state) => display_panel(self, ui, state.clone(), "Media Picker", mediapicker_ui),
            PanelId::Playlist => display_panel(self, ui, Arc::new(RwLock::new(String::new())), "Media Picker", playlist_ui),
        };
    }
}

impl Panel {
    pub fn new(id: PanelId, viewport_settings: ViewportBuilder) -> Self {
        Self {
            id,
            detached: Arc::new(AtomicBool::new(false)),
            toasts: Arc::new(Mutex::new(Toasts::new().direction(Direction::TopDown))),
            viewport_settings,
        }
    }
}

/// This function creates the default state of the panels
pub fn create_panels() -> Vec<Panel> {
    vec![Panel::new(
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
    )]
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
