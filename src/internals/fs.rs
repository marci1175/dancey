use std::{ffi::OsString, fs, path::PathBuf};

use crate::internals::utils::CacheState;

/// This is used to represent a folder in the filesystem.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FsMap {
    /// Should be the name of the folder
    pub name: OsString,
    /// The entries inside the folder
    pub objects: Vec<FsObject>,
}

/// The type of object a folder can contain
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FsObject {
    File {
        name: OsString,
        path: PathBuf,
    },
    Symlink(OsString),
    Folder {
        name: OsString,
        path: PathBuf,
        cache: CacheState<Option<FsMap>, ()>,
    },
}

pub fn create_entry_map(path: &PathBuf) -> anyhow::Result<FsMap> {
    let mut objects = vec![];
    let name = path
        .file_name()
        .ok_or(anyhow::Error::msg("Invalid path provided to map."))?
        .to_os_string();
    let dir = fs::read_dir(path)?;

    for entry in dir {
        let entry = entry?;

        // Check if the entry is a directory
        let ty = entry.file_type()?;

        if ty.is_dir() {
            // Recursively walk thorugh the directories
            objects.push(FsObject::Folder {
                name: entry.file_name(),
                path: entry.path(),
                cache: CacheState::NotReady(()),
            });
        }
        // Check if the entry is file
        else if ty.is_file() {
            objects.push(FsObject::File {
                name: entry.file_name(),
                path: entry.path(),
            });
        }
        // If the file is a symlink we should still display it but we should inform the user that its a symlink
        else {
            objects.push(FsObject::Symlink(entry.file_name()));
        }
    }

    Ok(FsMap { name, objects })
}
