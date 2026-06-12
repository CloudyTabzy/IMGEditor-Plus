use std::path::PathBuf;

use crate::parser::ImgVersion;

#[derive(Debug, Clone)]
pub struct SaveArchiveChoice {
    pub path: PathBuf,
    pub version: ImgVersion,
}

#[cfg(feature = "native-dialogs")]
pub fn open_file() -> anyhow::Result<Option<PathBuf>> {
    let path = rfd::FileDialog::new()
        .set_title("Open IMG archive")
        .add_filter("IMG Archive", &["img"])
        .pick_file();

    Ok(path)
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_file() -> anyhow::Result<Option<PathBuf>> {
    Ok(None)
}

#[cfg(feature = "native-dialogs")]
pub fn import_files() -> anyhow::Result<Vec<PathBuf>> {
    let paths = rfd::FileDialog::new()
        .set_title("Import files")
        .add_filter(
            "Importable files",
            &["dff", "txd", "col", "ifp", "ipl", "ide", "dat"],
        )
        .pick_files()
        .unwrap_or_default();

    Ok(paths)
}

#[cfg(not(feature = "native-dialogs"))]
pub fn import_files() -> anyhow::Result<Vec<PathBuf>> {
    Ok(Vec::new())
}

#[cfg(feature = "native-dialogs")]
pub fn save_archive(
    default_path: impl Into<PathBuf>,
    version: ImgVersion,
) -> anyhow::Result<Option<SaveArchiveChoice>> {
    let default_path = default_path.into();
    let file_name = default_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.img")
        .to_string();

    let path = rfd::FileDialog::new()
        .set_title("Save IMG archive")
        .add_filter("IMG Archive", &["img"])
        .set_file_name(file_name)
        .save_file();

    Ok(path.map(|path| SaveArchiveChoice { path, version }))
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_archive(
    default_path: impl Into<PathBuf>,
    version: ImgVersion,
) -> anyhow::Result<Option<SaveArchiveChoice>> {
    let _ = default_path.into();
    let _ = version;
    Ok(None)
}

#[cfg(feature = "native-dialogs")]
pub fn save_folder() -> anyhow::Result<Option<PathBuf>> {
    let path = rfd::FileDialog::new()
        .set_title("Select folder")
        .pick_folder();

    Ok(path)
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_folder() -> anyhow::Result<Option<PathBuf>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(feature = "native-dialogs"))]
    fn stubs_return_empty_results() {
        use crate::parser::ImgVersion;

        assert!(super::open_file().unwrap().is_none());
        assert!(super::import_files().unwrap().is_empty());
        assert!(
            super::save_archive("test.img", ImgVersion::One)
                .unwrap()
                .is_none()
        );
        assert!(super::save_folder().unwrap().is_none());
    }
}
