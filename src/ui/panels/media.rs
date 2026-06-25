use std::{ffi::OsStr, path::PathBuf, rc::Rc, sync::Arc};

use chrono::Utc;
use egui::{Color32, Context, Id, RichText, ScrollArea, Sense, Ui, UiBuilder};
use egui_toast::{ToastStyle, Toasts};
use indexmap::{IndexMap, IndexSet};
use parking_lot::{Mutex, RwLock};

use crate::{
    internals::{
        fs::{FsMap, create_entry_map},
        sample::{SampleProperties, fetch_sample_properties},
        utils::{CacheState, random_value},
    },
    ui::panels::{
        lib::{Panel, display_error_as_toast},
        playlist::DNDSampleInstance,
    },
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct BookmarkedObject {
    /// The string that gets displayed, the default alias of a file is its file name.
    /// This can be modified by the user.
    pub alias: String,
    /// Timestamp of when it was saved
    timestamp: chrono::DateTime<Utc>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BookmarkSelector {
    /// Bookmarks saved by the user
    /// We want to save order between the entries
    pub bookmarks: IndexMap<PathBuf, BookmarkedObject>,

    /// The object selected in the media selector
    pub selected_object: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FileSystemSelector {
    /// The path to the folder we have opened
    pub opened_folder: Option<PathBuf>,

    /// The object selected in the media selector
    pub selected_object: Option<PathBuf>,

    /// Current folder that has been read
    pub current_folder: FsMap,

    /// Currently dragged sample's properties
    #[serde(skip)]
    pub dragged_sample_props: SampleProperties,
}

/// State of the media selector panel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaPanel {
    /// State of the complete media selector widget
    pub media_selector_state: MediaSelectorState,

    /// The state of the FilesystemSelector
    pub filesystem_selector: FileSystemSelector,

    /// The state of the BookmarkSelector
    pub bookmark_selector: BookmarkSelector,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum MediaSelectorState {
    Bookmarks,
    FileSystem,
    Workspace,
}

/// This is what gets called when the panel is either attached or detached
pub fn mediapicker_ui(this: &Panel, ui: &mut Ui, state: Arc<RwLock<MediaPanel>>) {
    let current_state = state.read().clone();

    // Decide width of both objects
    let spacing = ui.spacing().item_spacing.x;
    const MEDIAPICKER_STATE_COUNT: f32 = 3.0;
    let width = (ui.available_width() - (spacing * (MEDIAPICKER_STATE_COUNT - 1.0)))
        / MEDIAPICKER_STATE_COUNT;

    // Create both buttons and make them take up all the space
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::selectable(
                    current_state.media_selector_state == MediaSelectorState::Workspace,
                    "Workspace",
                ),
            )
            .clicked()
        {
            state.write().media_selector_state = MediaSelectorState::Workspace;
        };

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
                    "Files",
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
        (match current_state.media_selector_state {
            MediaSelectorState::Bookmarks => 3,
            MediaSelectorState::FileSystem => 3,
            MediaSelectorState::Workspace => 1,
        } as f32)
    };

    // Decide width of all objects
    let spacing = ui.spacing().item_spacing.x * (toolbar_btn_count - 1.);
    let width = (ui.available_width() - spacing) / toolbar_btn_count;

    // Create a small toolbar for both media selectors
    ui.horizontal(|ui| {
        match current_state.media_selector_state {
            MediaSelectorState::Bookmarks => {
                ui.add_enabled_ui(
                    current_state.bookmark_selector.selected_object.is_some(),
                    |ui| {
                        if ui
                            .add_sized([width, 20.0], egui::Button::new("Add to Workspace"))
                            .clicked()
                        {
                            // Its safe to unwrap here due to the check above
                            // let selected_object = current_state
                            //     .bookmark_selector
                            //     .selected_object
                            //     .as_ref()
                            //     .unwrap();
                        };

                        if ui
                            .add_sized([width, 20.0], egui::Button::new("Remove"))
                            .clicked()
                        {
                            // Its safe to unwrap here due to the check above
                            let selected_object = current_state
                                .bookmark_selector
                                .selected_object
                                .as_ref()
                                .unwrap();

                            // Remove the bookmarked object from the bookmarks and reset the selected object field
                            state
                                .write()
                                .bookmark_selector
                                .bookmarks
                                .swap_remove(selected_object);
                            state.write().bookmark_selector.selected_object = None;
                        };

                        // Allocate a button in the pre calculated space
                        let edit_response = ui
                            .add_sized([width, 20.0], egui::Button::new("Edit"));

                        // Create a popup when the edit button is lcicked with the editing options available
                        egui::Popup::menu(&edit_response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).id(Id::new("bookmark_edit_button")).show(|ui| {
                            // Its safe to unwrap here due to the check above
                            let selected_object = current_state
                                .bookmark_selector
                                .selected_object
                                .as_ref()
                                .unwrap();

                            // Tell the user what theyre editing
                            ui.label("Alias");
                            ui.separator();

                            // Edit the alias of the bookmark
                            ui.text_edit_singleline(&mut state.write().bookmark_selector.bookmarks.get_mut(selected_object).unwrap().alias);
                        });
                    },
                );
            }
            MediaSelectorState::FileSystem => {
                // Only enable this button if we can bookmark an object
                ui.add_enabled_ui(
                    current_state.filesystem_selector.opened_folder.is_some()
                        && current_state.filesystem_selector.selected_object.is_some(),
                    |ui| {
                        if ui
                            .add_sized([width, 20.0], egui::Button::new("Add to Workspace"))
                            .clicked()
                        {};

                        if ui
                            .add_sized([width, 20.0], egui::Button::new("★"))
                            .clicked()
                        {
                            let path = current_state.filesystem_selector.selected_object.unwrap();

                            state.write().bookmark_selector.bookmarks.insert(
                                path.clone(),
                                BookmarkedObject {
                                    alias: path
                                        .file_name()
                                        .unwrap_or(OsStr::new("[Unable to acquire file name]"))
                                        .to_string_lossy()
                                        .to_string(),
                                    timestamp: chrono::Utc::now(),
                                },
                            );
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
                            state.write().filesystem_selector.current_folder = map;
                        }

                        // Save folder path
                        state.write().filesystem_selector.opened_folder = Some(folder);
                    }
                };
            }
            MediaSelectorState::Workspace => {}
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
                    let selected_bookmark_entry =
                        &mut state.write().bookmark_selector.selected_object;

                    for (path, bookmarks) in current_state.bookmark_selector.bookmarks {
                        let entry = ui
                            .scope(|ui| {
                                ui.style_mut().interaction.selectable_labels = false;

                                ui.label(
                                    RichText::from(
                                        bookmarks.alias,
                                    )
                                    .strong()
                                    .background_color({
                                        // Highlight the label if the user has clicked on it
                                        if *selected_bookmark_entry != Some(path.clone()) {
                                            Color32::TRANSPARENT
                                        } else {
                                            Color32::GRAY
                                        }
                                    }),
                                )
                                .interact(Sense::click_and_drag())
                            })
                            .inner;

                        if entry.clicked() {
                            // Modify the selected object variable, if it has been re-selected reset the value.
                            if *selected_bookmark_entry != Some(path.clone()) {
                                *selected_bookmark_entry = Some(path.clone());
                            } else {
                                *selected_bookmark_entry = None;
                            }
                        };

                        entry.context_menu(|ui| {
                            ui.label(RichText::from(path.to_string_lossy()).weak());
                        });
                    }
                }
                // Draw file system selector
                MediaSelectorState::FileSystem => {
                    ui.separator();

                    ui.label(
                        RichText::from(
                            current_state
                                .filesystem_selector
                                .current_folder
                                .name
                                .to_string_lossy(),
                        )
                        .strong(),
                    );

                    ui.separator();

                    // Borrow mutably
                    let file_system_selector = &mut state.write().filesystem_selector;

                    // Display the mapped folder
                    ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            display_filesystem_map(
                                &mut file_system_selector.current_folder,
                                ui,
                                &mut file_system_selector.selected_object,
                                &mut file_system_selector.dragged_sample_props,
                                this.toasts.clone(),
                            );
                        });
                }
                MediaSelectorState::Workspace => {}
            }
        },
    );
}

/// Display the map of directory items.
fn display_filesystem_map(
    map: &mut FsMap,
    ui: &mut Ui,
    selected_object: &mut Option<PathBuf>,
    dragged_sample_props: &mut SampleProperties,
    toasts: Arc<Mutex<Toasts>>,
) {
    for entry in &mut map.objects {
        match entry {
            crate::internals::fs::FsObject::File { name, path } => {
                // Create an entry where the users cannot copy the text from it directly
                // Make this object draggable and the payload should the the path of the object we are referencing in the ui.
                let entry = ui.dnd_drag_source(
                    Id::new(&*path),
                    DNDSampleInstance {
                        name: name.clone(),
                        color: Color32::from_rgba_unmultiplied(255, 255, 255, 120),
                        properties: dragged_sample_props.clone(),
                    },
                    |ui| {
                        ui.scope(|ui| {
                            // Set this so we cannot select text
                            ui.style_mut().interaction.selectable_labels = false;

                            // Display the actual label
                            ui.label(
                                RichText::from(name.to_string_lossy())
                                    .strong()
                                    .background_color({
                                        // Highlight the label if the user has clicked on it
                                        if *selected_object != Some(path.clone()) {
                                            Color32::TRANSPARENT
                                        } else {
                                            Color32::GRAY
                                        }
                                    }),
                            )
                        })
                    },
                );

                // Catch both clicks and dragging in the ui
                let entry_response = ui.interact(
                    entry.response.rect,
                    Id::new(&*path),
                    Sense::click_and_drag(),
                );

                // Sense if the label is being clicked on
                if entry_response.clicked() {
                    // Modify the selected object variable, if it has been re-selected reset the value.
                    if *selected_object != Some(path.clone()) {
                        *selected_object = Some(path.clone());
                    } else {
                        *selected_object = None;
                    }
                };

                if entry_response.drag_started() {
                    // Fetch information about the file we are dragging such as length, sample rate, etc. These will be used when inserted into the playlist.
                    if let Some(props) = display_error_as_toast(
                        fetch_sample_properties(&*path),
                        ToastStyle::default(),
                        toasts.clone(),
                    ) {
                        *dragged_sample_props = props;
                    } else {
                        ui.ctx().stop_dragging();
                        ui.ctx().set_cursor_icon(egui::CursorIcon::default());
                    }
                }
            }
            crate::internals::fs::FsObject::Symlink(os_string) => {
                ui.label(RichText::from(os_string.to_string_lossy()).weak())
                    .on_hover_text("This file is a symlink.");
            }
            crate::internals::fs::FsObject::Folder { name, path, cache } => {
                ui.collapsing(name.to_string_lossy(), |ui| match cache {
                    CacheState::Ready(map_result) => match map_result {
                        // Display the result of the read
                        Some(entries) => display_filesystem_map(
                            entries,
                            ui,
                            selected_object,
                            dragged_sample_props,
                            toasts.clone(),
                        ),
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
