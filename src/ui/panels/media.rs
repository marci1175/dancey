use std::{ffi::OsStr, path::PathBuf, sync::Arc};

use chrono::Utc;
use egui::{Color32, Id, Response, RichText, ScrollArea, Sense, Ui, UiBuilder};
use egui_toast::{ToastStyle, Toasts};
use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};

use crate::{
    internals::{
        fs::{FsMap, create_entry_map},
        sample::{SampleProperties, fetch_sample_properties},
        utils::{CacheState, random_value},
    },
    ui::panels::{
        lib::{Panel, PanelStates, display_error_as_toast},
        playlist::SampleInstance,
    },
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct BookmarkedObject {
    /// The string that gets displayed, the default alias of a file is its file name.
    /// This can be modified by the user.
    pub alias: String,
    /// Timestamp of when it was saved
    pub timestamp: chrono::DateTime<Utc>,
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

    /// The object selected in the file system selector
    pub selected_object: Option<PathBuf>,

    /// Current folder that has been read
    pub current_folder: FsMap,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct WorkspaceSampleAttributes {
    pub alias: String,

    pub color: Color32,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSelector {
    pub workspace_samples: IndexMap<PathBuf, WorkspaceSampleAttributes>,

    /// The object selected in the workspace selector
    pub selected_object: Option<PathBuf>,
}

/// State of the media selector panel.
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaPanel {
    /// State of the complete media selector widget
    pub media_selector_state: MediaSelectorState,

    /// The state of the FilesystemSelector
    pub filesystem_selector: FileSystemSelector,

    /// These are dependent on the specific workspace we are working in so we can skip saving this. (This is what bookmarks are for)
    #[serde(skip)]
    pub workspace_selector: WorkspaceSelector,

    /// The state of the BookmarkSelector
    pub bookmark_selector: BookmarkSelector,

    /// Currently dragged sample's properties, this is global among all selectors
    #[serde(skip)]
    pub dragged_sample_props: SampleProperties,
}

#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum MediaSelectorState {
    Bookmarks,
    #[default]
    FileSystem,
    Workspace,
}

/// This is what gets called when the panel is either attached or detached
pub fn mediapicker_ui(
    this: &Panel,
    ui: &mut Ui,
    state: Arc<RwLock<MediaPanel>>,
    _global_state: PanelStates,
) {
    let media_selector_state = state.read().media_selector_state;

    picker_type_selector(ui, state.clone(), &media_selector_state);

    ui.separator();

    picker_toolbar(this, ui, state.clone(), media_selector_state);

    ui.separator();

    // Paint the rest of the ui with black and display the media selector here.
    // We should handle both states of the media selector
    ui.painter_at(ui.available_rect_before_wrap()).rect_filled(
        ui.available_rect_before_wrap(),
        5.,
        Color32::BLACK,
    );

    // Allocate the ui for the media selector type
    ui.scope_builder(
        UiBuilder::new().max_rect(ui.available_rect_before_wrap()),
        |ui| {
            // Handle both states of the mediaselector
            match media_selector_state {
                MediaSelectorState::Bookmarks => {
                    let bookmarks = state.read().bookmark_selector.bookmarks.clone();
                    let mut guard = state.write();

                    // Split the guarded state into disjoint mutable borrows up front.
                    let state_ref = &mut *guard;
                    let selected_object = &mut state_ref.bookmark_selector.selected_object;
                    let dragged_sample_props = &mut state_ref.dragged_sample_props;

                    for (path, bookmark) in bookmarks {
                        let entry = draggable_sample(
                            ui,
                            selected_object,
                            dragged_sample_props,
                            this.toasts.clone(),
                            bookmark.alias.into(),
                            path.clone(),
                        );
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
                            state
                                .read()
                                .filesystem_selector
                                .current_folder
                                .name
                                .to_string_lossy(),
                        )
                        .strong(),
                    );

                    ui.separator();

                    // Borrow mutably
                    let mut guard = state.write();

                    // Split the guarded state into disjoint mutable borrows up front,
                    // so the borrow checker can see each field is independent.
                    let state_ref = &mut *guard;
                    let filesystem_selector = &mut state_ref.filesystem_selector;
                    let dragged_sample_props = &mut state_ref.dragged_sample_props;

                    // Split filesystem_selector's fields disjointly too.
                    let current_folder = &mut filesystem_selector.current_folder;
                    let selected_object = &mut filesystem_selector.selected_object;

                    // Display the mapped folder
                    ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            display_filesystem_map(
                                ui,
                                current_folder,
                                selected_object,
                                dragged_sample_props,
                                this.toasts.clone(),
                            );
                        });
                }
                MediaSelectorState::Workspace => {
                    let samples = state.read().workspace_selector.workspace_samples.clone();
                    let mut guard = state.write();

                    // Split the guarded state into disjoint mutable borrows up front.
                    let state_ref = &mut *guard;
                    let selected_object = &mut state_ref.workspace_selector.selected_object;
                    let dragged_sample_props = &mut state_ref.dragged_sample_props;

                    for (path, sample) in samples {
                        let entry = draggable_sample(
                            ui,
                            selected_object,
                            dragged_sample_props,
                            this.toasts.clone(),
                            sample.alias.into(),
                            path.clone(),
                        );
                        entry.context_menu(|ui| {
                            ui.label(RichText::from(path.to_string_lossy()).weak());
                        });
                    }
                }
            }
        },
    );
}

fn picker_toolbar(
    this: &Panel,
    ui: &mut Ui,
    state: Arc<parking_lot::lock_api::RwLock<parking_lot::RawRwLock, MediaPanel>>,
    media_selector_state: MediaSelectorState,
) {
    // Make sure to update the object count so that all widgets are properly sized.
    let toolbar_btn_count: f32 = {
        (match media_selector_state {
            MediaSelectorState::Bookmarks => 3,
            MediaSelectorState::FileSystem => 3,
            MediaSelectorState::Workspace => 2,
        } as f32)
    };

    // Decide width of all objects
    let spacing = ui.spacing().item_spacing.x * (toolbar_btn_count - 1.);
    let width = (ui.available_width() - spacing) / toolbar_btn_count;

    // Create a small toolbar for both media selectors
    ui.horizontal(|ui| {
        match media_selector_state {
            MediaSelectorState::Bookmarks => {
                let bookmark_selector = state.read().bookmark_selector.clone();

                ui.add_enabled_ui(bookmark_selector.selected_object.is_some(), |ui| {
                    if ui
                        .add_sized([width, 20.0], egui::Button::new("Add to Workspace"))
                        .clicked()
                    {
                        // Its safe to unwrap here due to the check above
                        let selected_object = bookmark_selector.selected_object.as_ref().unwrap();

                        // Fetch the selected object from the objects
                        // Safe to unwrap here since the object cannot be selected if it has been removed or not present in the list.
                        let (_idx, path, attr) = bookmark_selector
                            .bookmarks
                            .get_full(selected_object)
                            .unwrap();

                        // Store in the workspace tab
                        state
                            .write()
                            .workspace_selector
                            .workspace_samples
                            .insert_full(
                                path.clone(),
                                WorkspaceSampleAttributes {
                                    alias: attr.alias.clone(),
                                    color: random_color_with_opacity(),
                                },
                            );
                    };

                    if ui
                        .add_sized([width, 20.0], egui::Button::new("Remove"))
                        .clicked()
                    {
                        // Its safe to unwrap here due to the check above
                        let selected_object = bookmark_selector.selected_object.as_ref().unwrap();

                        // Remove the bookmarked object from the bookmarks and reset the selected object field
                        state
                            .write()
                            .bookmark_selector
                            .bookmarks
                            .swap_remove(selected_object);
                        state.write().bookmark_selector.selected_object = None;
                    };

                    // Allocate a button in the pre calculated space
                    let edit_response = ui.add_sized([width, 20.0], egui::Button::new("Edit"));

                    // Create a popup when the edit button is lcicked with the editing options available
                    egui::Popup::menu(&edit_response)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| {
                            // Its safe to unwrap here due to the check above
                            let selected_object =
                                bookmark_selector.selected_object.as_ref().unwrap();

                            // Tell the user what theyre editing
                            ui.label("Alias");
                            ui.separator();

                            // Edit the alias of the bookmark
                            ui.text_edit_singleline(
                                &mut state
                                    .write()
                                    .bookmark_selector
                                    .bookmarks
                                    .get_mut(selected_object)
                                    .unwrap()
                                    .alias,
                            );
                        });
                });
            }
            MediaSelectorState::FileSystem => {
                let filesystem_selector = state.read().filesystem_selector.clone();

                // Only enable this button if we can bookmark an object
                ui.add_enabled_ui(
                    filesystem_selector.opened_folder.is_some()
                        && filesystem_selector.selected_object.is_some(),
                    |ui| {
                        if ui
                            .add_sized([width, 20.0], egui::Button::new("Add to Workspace"))
                            .clicked()
                        {
                            // Its safe to unwrap here due to the check above
                            let selected_object =
                                filesystem_selector.selected_object.as_ref().unwrap();

                            // Store in the workspace tab
                            state
                                .write()
                                .workspace_selector
                                .workspace_samples
                                .insert_full(
                                    selected_object.clone(),
                                    WorkspaceSampleAttributes {
                                        alias: selected_object
                                            .file_name()
                                            .unwrap_or(OsStr::new("[Unable to acquire file name]"))
                                            .to_string_lossy()
                                            .to_string(),
                                        color: random_color_with_opacity(),
                                    },
                                );
                        };

                        if ui
                            .add_sized([width, 20.0], egui::Button::new("★"))
                            .clicked()
                        {
                            let path = filesystem_selector.selected_object.as_ref().unwrap();

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
            MediaSelectorState::Workspace => {
                let workspace_selector = state.read().workspace_selector.clone();

                ui.add_enabled_ui(workspace_selector.selected_object.is_some(), |ui| {
                    if ui
                        .add_sized([width, 20.0], egui::Button::new("Remove"))
                        .clicked()
                    {
                        // Its safe to unwrap here due to the check above
                        let selected_object = workspace_selector.selected_object.as_ref().unwrap();

                        // Remove the bookmarked object from the bookmarks and reset the selected object field
                        state
                            .write()
                            .workspace_selector
                            .workspace_samples
                            .swap_remove(selected_object);
                        state.write().workspace_selector.selected_object = None;
                    };

                    // Allocate a button in the pre calculated space
                    let edit_response = ui.add_sized([width, 20.0], egui::Button::new("Edit"));

                    // Create a popup when the edit button is lcicked with the editing options available
                    egui::Popup::menu(&edit_response)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| {
                            // Its safe to unwrap here due to the check above
                            let selected_object =
                                workspace_selector.selected_object.as_ref().unwrap();

                            // Tell the user what theyre editing
                            ui.label("Alias");
                            ui.separator();

                            // Edit the alias of the bookmark
                            ui.text_edit_singleline(
                                &mut state
                                    .write()
                                    .workspace_selector
                                    .workspace_samples
                                    .get_mut(selected_object)
                                    .unwrap()
                                    .alias,
                            );
                        });
                });
            }
        }
    });
}

fn random_color_with_opacity() -> Color32 {
    Color32::from_rgba_unmultiplied(random_value(), random_value(), random_value(), 120)
}

fn picker_type_selector(
    ui: &mut Ui,
    state: Arc<RwLock<MediaPanel>>,
    media_selector_state: &MediaSelectorState,
) {
    // Decide width of both objects
    let spacing = ui.spacing().item_spacing.x;
    const MEDIAPICKER_STATE_COUNT: f32 = 3.0;
    let width = (ui.available_width() - (spacing * (MEDIAPICKER_STATE_COUNT - 1.0)))
        / MEDIAPICKER_STATE_COUNT;

    let selector_state = &mut state.write().media_selector_state;

    // Create both buttons and make them take up all the space
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::selectable(
                    *media_selector_state == MediaSelectorState::Workspace,
                    "Workspace",
                ),
            )
            .clicked()
        {
            *selector_state = MediaSelectorState::Workspace;
        };

        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::selectable(
                    *media_selector_state == MediaSelectorState::Bookmarks,
                    "Bookmarks",
                ),
            )
            .clicked()
        {
            *selector_state = MediaSelectorState::Bookmarks;
        };

        if ui
            .add_sized(
                [width, 20.0],
                egui::Button::selectable(
                    *media_selector_state == MediaSelectorState::FileSystem,
                    "Files",
                ),
            )
            .clicked()
        {
            *selector_state = MediaSelectorState::FileSystem;
        };
    });
}

/// Display the map of directory items.
fn display_filesystem_map(
    ui: &mut Ui,
    map: &mut FsMap,
    selected_object: &mut Option<PathBuf>,
    dragged_sample_props: &mut SampleProperties,
    toasts: Arc<Mutex<Toasts>>,
) {
    for entry in &mut map.objects {
        match entry {
            crate::internals::fs::FsObject::File { name, path } => {
                // Create an entry where the users cannot copy the text from it directly
                // Make this object draggable and the payload should the the path of the object we are referencing in the ui.
                draggable_sample(
                    ui,
                    selected_object,
                    dragged_sample_props,
                    toasts.clone(),
                    name.clone(),
                    path.clone(),
                );
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
                            ui,
                            entries,
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

fn draggable_sample(
    ui: &mut Ui,
    selected_object: &mut Option<PathBuf>,
    dragged_sample_props: &mut SampleProperties,
    toasts: Arc<Mutex<Toasts>>,
    name: std::ffi::OsString,
    path: PathBuf,
) -> Response {
    let entry = draggable_sample_label(
        ui,
        &*selected_object,
        dragged_sample_props.clone(),
        name.clone(),
        path.clone(),
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
            fetch_sample_properties(&path),
            ToastStyle::default(),
            toasts.clone(),
        ) {
            *dragged_sample_props = props;
        } else {
            ui.ctx().stop_dragging();
            ui.ctx().set_cursor_icon(egui::CursorIcon::default());
        }
    }

    entry_response
}

fn draggable_sample_label(
    ui: &mut Ui,
    selected_object: &Option<PathBuf>,
    dragged_sample_props: SampleProperties,
    name: std::ffi::OsString,
    path: PathBuf,
) -> egui::InnerResponse<egui::InnerResponse<egui::Response>> {
    ui.dnd_drag_source(
        Id::new(&*path),
        SampleInstance {
            name: name.clone(),
            color: Color32::from_rgba_unmultiplied(255, 255, 255, 120),
            path: path.clone(),
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
    )
}
