use std::{ffi::OsStr, path::PathBuf, sync::Arc};

use chrono::Utc;
use egui::{Color32, Id, RichText, ScrollArea, Sense, Ui, UiBuilder};
use egui_toast::ToastStyle;
use indexmap::IndexSet;
use parking_lot::RwLock;

use crate::{
    internals::{
        fs::{create_entry_map, FsMap},
        utils::CacheState,
    },
    ui::panels::lib::{display_error_as_toast, Panel},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct BookmarkedObject {
    /// Timestamp of when it was saved
    timestamp: chrono::DateTime<Utc>,

    /// Path to the object
    path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FileSystemSelector {
    /// The path to the folder we have opened
    pub opened_folder: Option<PathBuf>,
    /// The object selected in the media selector
    pub selected_object: Option<PathBuf>,
    /// Current folder that has been read
    pub current_folder: FsMap,
}

/// State of the media selector panel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaPanel {
    /// State of the actual media selector widget
    pub media_selector_state: MediaSelectorState,

    /// Bookmarks saved by the user
    /// We want to save order between the entries
    pub bookmarks: IndexSet<BookmarkedObject>,

    /// The state of the FilesystemSelector
    pub filesystem_selector_state: FileSystemSelector,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum MediaSelectorState {
    Bookmarks,
    FileSystem,
}

/// This is what gets called when the panel is either attached or detached
pub fn mediapicker_ui(this: &Panel, ui: &mut Ui, state: Arc<RwLock<MediaPanel>>) {
    let current_state = state.read().clone();

    // Decide width of both objects
    let spacing = ui.spacing().item_spacing.x;
    const MEDIAPICKER_STATE_COUNT: f32 = 2.0;
    let width = (ui.available_width() - spacing) / MEDIAPICKER_STATE_COUNT;

    // Create both buttons and make them take up all the space
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::selectable(
                    current_state.media_selector_state == MediaSelectorState::Bookmarks,
                    "Bookmarks",
                ),
            )
            .clicked()
        {
            state.write().media_selector_state = MediaSelectorState::Bookmarks;
        };

        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::selectable(
                    current_state.media_selector_state == MediaSelectorState::FileSystem,
                    "FileSystem",
                ),
            )
            .clicked()
        {
            state.write().media_selector_state = MediaSelectorState::FileSystem;
        };
    });

    ui.separator();

    // Make sure to update the object count so that all widgets are properly sized.
    let toolbar_btn_count: f32 = {
        match current_state.media_selector_state {
            MediaSelectorState::Bookmarks => 3.0,
            MediaSelectorState::FileSystem => 2.0,
        }
    };

    // Decide width of all objects
    let spacing = ui.spacing().item_spacing.x * (toolbar_btn_count - 1.);
    let width = (ui.available_width() - spacing) / toolbar_btn_count;

    // Create a small toolbar for both media selectors
    ui.horizontal(|ui| {
        match current_state.media_selector_state {
            MediaSelectorState::Bookmarks => {
                if ui
                    .add_sized([width, 20.0], egui::Button::new("Add"))
                    .clicked()
                {};
                if ui
                    .add_sized([width, 20.0], egui::Button::new("Remove"))
                    .clicked()
                {};
                if ui
                    .add_sized([width, 20.0], egui::Button::new("Edit"))
                    .clicked()
                {};
            }
            MediaSelectorState::FileSystem => {
                // Only enable this button if we can bookmark an object
                ui.add_enabled_ui(
                    current_state
                        .filesystem_selector_state
                        .opened_folder
                        .is_some()
                        && current_state
                            .filesystem_selector_state
                            .selected_object
                            .is_some(),
                    |ui| {
                        if ui
                            .add_sized([width, 20.0], egui::Button::new("★"))
                            .clicked()
                        {
                            state.write().bookmarks.insert(BookmarkedObject {
                                timestamp: chrono::Utc::now(),
                                // Its safe to unwrap here since we disable the ui if this is None.
                                path: current_state
                                    .filesystem_selector_state
                                    .selected_object
                                    .unwrap(),
                            });
                        };
                    },
                );

                if ui
                    .add_sized([width, 20.0], egui::Button::new("Open Folder"))
                    .clicked()
                {
                    // Only update the selected folder if the folder we selected is valid
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        // Recursively iterate thorugh every folder entry and create a map of items.
                        if let Some(map) = display_error_as_toast(
                            create_entry_map(&folder),
                            ToastStyle::default(),
                            this.toasts.clone(),
                        ) {
                            // Save mapped folder
                            state.write().filesystem_selector_state.current_folder = map;
                        }

                        // Save folder path
                        state.write().filesystem_selector_state.opened_folder = Some(folder);
                    }
                };
            }
        }
    });

    ui.separator();

    // Paint the rest of the ui with black and display the media selector here.
    // We should handle both states of the media selector
    ui.painter_at(ui.available_rect_before_wrap()).rect_filled(
        ui.available_rect_before_wrap(),
        5.,
        Color32::BLACK,
    );

    // Allocate the ui for the mediaselector
    ui.scope_builder(
        UiBuilder::new().max_rect(ui.available_rect_before_wrap()),
        |ui| {
            // Handle both states of the mediaselector
            match current_state.media_selector_state {
                MediaSelectorState::Bookmarks => {
                    for bookmark in current_state.bookmarks {
                        let entry = ui
                            .scope(|ui| {
                                ui.style_mut().interaction.selectable_labels = false;

                                ui.label(
                                    RichText::from(
                                        bookmark
                                            .path
                                            .file_name()
                                            .unwrap_or(OsStr::new("[Unable to acquire file name]"))
                                            .to_string_lossy(),
                                    )
                                    .strong(),
                                )
                                .interact(Sense::click_and_drag())
                            })
                            .inner;

                        entry.context_menu(|ui| {
                            ui.label(RichText::from(bookmark.path.to_string_lossy()).weak());
                        });
                    }
                }
                // Draw file system selector
                MediaSelectorState::FileSystem => {
                    ui.separator();

                    ui.label(
                        RichText::from(
                            current_state
                                .filesystem_selector_state
                                .current_folder
                                .name
                                .to_string_lossy(),
                        )
                        .strong(),
                    );

                    ui.separator();

                    // Borrow mutably
                    let file_system_selector = &mut state.write().filesystem_selector_state;

                    // Display the mapped folder
                    ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            display_filesystem_map(
                                &mut file_system_selector.current_folder,
                                ui,
                                &mut file_system_selector.selected_object,
                            );
                        });
                }
            }
        },
    );
}

/// Display the map of directory items.
fn display_filesystem_map(map: &mut FsMap, ui: &mut Ui, selected_object: &mut Option<PathBuf>) {
    for entry in &mut map.objects {
        match entry {
            crate::internals::fs::FsObject::File { name, path } => {
                // Create an entry where the users cannot copy the text from it directly
                // Make this object draggable and the payload should the the path of the object we are referencing in the ui.
                let entry = ui.dnd_drag_source(Id::new(&*path), path.clone(), |ui| {
                    ui.scope(|ui| {
                        // Set this so we cannot select text
                        ui.style_mut().interaction.selectable_labels = false;

                        // Display the actual label
                        ui.label(RichText::from(name.to_string_lossy()).background_color({
                            // Highlight the label if the user has clicked on it
                            if *selected_object != Some(path.clone()) {
                                Color32::TRANSPARENT
                            } else {
                                Color32::GRAY
                            }
                        }))
                        .interact(Sense::click_and_drag())
                    })
                });

                let click = ui.interact(
                    entry.response.rect,
                    Id::new(&*path).with("click"),
                    Sense::click(),
                );

                // Sense if the label is being clicked on
                if click.clicked() {
                    // Modify the selected object variable, if it has been re-selected reset the value.
                    if *selected_object != Some(path.clone()) {
                        *selected_object = Some(path.clone());
                    } else {
                        *selected_object = None;
                    }
                };
            }
            crate::internals::fs::FsObject::Symlink(os_string) => {
                ui.label(RichText::from(os_string.to_string_lossy()).weak())
                    .on_hover_text("This file is a symlink.");
            }
            crate::internals::fs::FsObject::Folder {
                name,
                path,
                ref mut cache,
            } => {
                ui.collapsing(name.to_string_lossy(), |ui| match cache {
                    CacheState::Ready(map_result) => match map_result {
                        // Display the result of the read
                        Some(entries) => display_filesystem_map(entries, ui, selected_object),
                        // If we failed to load the directory in we can always retry
                        None => {
                            ui.horizontal(|ui| {
                                ui.label("Failed to read directory.");
                                if ui.button("Retry").clicked() {
                                    *cache = CacheState::Ready(create_entry_map(&*path).ok());
                                }
                            });
                        }
                    },
                    // If the folder has been loaded in yet, load it in now. (All folder maps are lazy)
                    CacheState::NotReady(()) => {
                        *cache = CacheState::Ready(create_entry_map(&*path).ok());
                    }
                });
            }
        }
    }
}
