use std::path::PathBuf;

use windows::Win32::System::LibraryLoader::LoadLibraryW;

use crate::internals::mem::string_to_pcwstr;

pub fn load_library(path: PathBuf) -> anyhow::Result<()> {
    let (str, _chars) = string_to_pcwstr(dbg!(&path.to_string_lossy()));

    unsafe { LoadLibraryW(str) }?;

    Ok(())
}