use std::path::PathBuf;

use iced::Task;

use crate::parser::ImgVersion;

#[derive(Debug, Clone)]
pub struct SaveArchiveChoice {
    pub path: PathBuf,
    pub version: ImgVersion,
}

#[cfg(feature = "native-dialogs")]
pub fn open_file() -> Task<Option<PathBuf>> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Open IMG archive")
                .add_filter("IMG Archive", &["img"])
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |path| path,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_file() -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(feature = "native-dialogs")]
pub fn import_files() -> Task<Vec<PathBuf>> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Import files")
                .add_filter(
                    "Importable files",
                    &["dff", "txd", "col", "ifp", "ipl", "ide", "dat"],
                )
                .pick_files()
                .await
                .map(|handles| {
                    handles
                        .into_iter()
                        .map(|h| h.path().to_path_buf())
                        .collect()
                })
                .unwrap_or_default()
        },
        |paths| paths,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn import_files() -> Task<Vec<PathBuf>> {
    Task::none()
}

#[cfg(feature = "native-dialogs")]
pub fn save_archive(default_path: PathBuf, version: ImgVersion) -> Task<Option<SaveArchiveChoice>> {
    let file_name = default_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.img")
        .to_string();
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title("Save IMG archive")
                .add_filter("IMG Archive", &["img"])
                .set_file_name(file_name)
                .save_file()
                .await
                .map(|handle| SaveArchiveChoice {
                    path: handle.path().to_path_buf(),
                    version,
                })
        },
        |choice| choice,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_archive(_default_path: PathBuf, _version: ImgVersion) -> Task<Option<SaveArchiveChoice>> {
    Task::none()
}

#[cfg(feature = "native-dialogs")]
pub fn save_folder() -> Task<Option<PathBuf>> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Select export folder")
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |folder| folder,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_folder() -> Task<Option<PathBuf>> {
    Task::none()
}
