use std::sync::{atomic::AtomicBool, Arc};

use egui::{Context, InnerResponse, ViewportBuilder};

use crate::ui::panels::{media::display_media, root::display_root};

#[derive(Debug, Clone, Copy, Hash, serde::Serialize, serde::Deserialize)]
/// Enum for labeling all the different panels of the application
pub enum PanelId {
    /// Acts as the root for the application, this is the lowest layer of ui.
    Root,

    /// Media selector
    Media,
}

/// A dedicated portion of the ui.
/// All panels are detachable into children uis.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Panel {
    /// Specifies the type of the panel.
    pub id: PanelId,

    /// Tells if the panel is detached from the root ui.
    /// This field is thread safe, because we might need to access this from the child window.
    pub detached: Arc<AtomicBool>,

    #[serde(skip)]
    /// Viewport settings for the detached window.
    pub viewport_settings: ViewportBuilder,
}

impl Panel {
    pub fn display(&mut self, ctx: &Context) -> Option<InnerResponse<()>> {
        match self.id {
            PanelId::Root => display_root(self, ctx),
            PanelId::Media => display_media(self, ctx),
        }
    }
}

impl Panel {
    pub fn new(id: PanelId, viewport_settings: ViewportBuilder) -> Self {
        Self {
            id,
            detached: Arc::new(AtomicBool::new(false)),
            viewport_settings,
        }
    }
}

pub fn create_panels() -> Vec<Panel> {
    vec![Panel::new(
        PanelId::Media,
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
        },
    )]
}
