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
                .add_filter("IMG Archive", &["img", "dir"])
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |path| path,
    )
}

/// Pick an entry-list manifest for archive comparison. The remembered
/// directory is only used when it still exists; rfd otherwise falls back to
/// its normal platform location.
#[cfg(feature = "native-dialogs")]
pub fn open_compare_manifest(directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    Task::perform(
        async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Compare with entry list")
                .add_filter("IMG entry list", &["img.compare", "compare"]);
            if let Some(directory) = directory.filter(|path| path.is_dir()) {
                dialog = dialog.set_directory(directory);
            }
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |path| path,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_compare_manifest(_directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_file() -> Task<Option<PathBuf>> {
    Task::none()
}

/// Pick a loose Bully animation group (`Anim/*.agr`). The model still comes
/// from the open archive; only the animation is read from disk.
#[cfg(feature = "native-dialogs")]
pub fn open_agr_file() -> Task<Option<PathBuf>> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Open Bully animation group")
                .add_filter("Bully animation group", &["agr"])
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |path| path,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_agr_file() -> Task<Option<PathBuf>> {
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

#[cfg(feature = "native-dialogs")]
pub fn import_folder() -> Task<Option<PathBuf>> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Select folder to import")
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |folder| folder,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn import_folder() -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(not(feature = "native-dialogs"))]
pub fn import_files() -> Task<Vec<PathBuf>> {
    Task::none()
}

/// Pick a source image for texture replacement / TXD authoring.
#[cfg(feature = "native-dialogs")]
pub fn pick_image_file() -> Task<Option<PathBuf>> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Choose an image")
                .add_filter("Images", &["png", "dds", "bmp", "tga"])
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |path| path,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn pick_image_file() -> Task<Option<PathBuf>> {
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

/// Pick the destination for an exported entry-list manifest.
#[cfg(feature = "native-dialogs")]
pub fn save_compare_manifest(
    default_path: PathBuf,
    directory: Option<PathBuf>,
) -> Task<Option<PathBuf>> {
    let file_name = default_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.img.compare")
        .to_string();
    Task::perform(
        async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Export entry list")
                .add_filter("IMG entry list", &["img.compare", "compare"])
                .set_file_name(file_name);
            if let Some(directory) = directory.filter(|path| path.is_dir()) {
                dialog = dialog.set_directory(directory);
            }
            dialog
                .save_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
        |path| path,
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_compare_manifest(
    _default_path: PathBuf,
    _directory: Option<PathBuf>,
) -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_archive(
    _default_path: PathBuf,
    _version: ImgVersion,
) -> Task<Option<SaveArchiveChoice>> {
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
