use std::path::PathBuf;

pub fn open_file() -> anyhow::Result<Option<PathBuf>> {
    Ok(None)
}

pub fn import_files() -> anyhow::Result<Vec<PathBuf>> {
    Ok(Vec::new())
}

pub fn save_archive(default_path: impl Into<PathBuf>) -> anyhow::Result<Option<PathBuf>> {
    let _ = default_path.into();
    Ok(None)
}

pub fn save_folder() -> anyhow::Result<Option<PathBuf>> {
    Ok(None)
}
