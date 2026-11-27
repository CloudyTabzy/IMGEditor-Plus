#[cfg(feature = "native-dialogs")]
use std::future::Future;
#[cfg(feature = "native-dialogs")]
use std::path::Path;
use std::path::PathBuf;

use iced::Task;

#[cfg(feature = "native-dialogs")]
use crate::i18n::t;
use crate::parser::ImgVersion;

#[derive(Debug, Clone)]
pub struct SaveArchiveChoice {
    pub path: PathBuf,
    pub version: ImgVersion,
}

/// Runs a native `rfd::AsyncFileDialog` parented to the active Iced window.
///
/// Extracts the native window handle briefly on the UI thread via
/// [`iced::window::run`] (using `rfd` 0.18's `?Sized` `set_parent`), then
/// awaits the dialog on Iced's Tokio runtime while `rfd` runs its own STA COM
/// thread. Parenting keeps the dialog window-modal and guarantees it stays
/// above our window even when Always on Top (`WS_EX_TOPMOST`) is active.
#[cfg(feature = "native-dialogs")]
fn run_dialog<T, B, F, Fut>(build: B, run: F) -> Task<T>
where
    T: Send + 'static,
    B: FnOnce() -> rfd::AsyncFileDialog + Send + 'static,
    F: FnOnce(rfd::AsyncFileDialog) -> Fut + Send + 'static,
    Fut: Future<Output = T> + Send + 'static,
{
    let mut build = Some(build);
    let mut run = Some(run);
    iced::window::latest().then(move |window_id| {
        let (Some(build), Some(run)) = (build.take(), run.take()) else {
            return Task::none();
        };
        let dialog = build();
        match window_id {
            Some(id) => {
                let mut run = Some(run);
                iced::window::run(id, move |window| dialog.set_parent(window)).then(move |dialog| {
                    match run.take() {
                        Some(run) => Task::future(run(dialog)),
                        None => Task::none(),
                    }
                })
            }
            None => Task::future(run(dialog)),
        }
    })
}

#[cfg(feature = "native-dialogs")]
fn apply_existing_directory(
    dialog: rfd::AsyncFileDialog,
    directory: Option<PathBuf>,
) -> rfd::AsyncFileDialog {
    if let Some(directory) = directory.filter(|path| path.is_dir()) {
        dialog.set_directory(directory)
    } else {
        dialog
    }
}

#[cfg(feature = "native-dialogs")]
fn existing_parent_dir(path: &Path) -> Option<PathBuf> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty() && parent.is_dir())
        .map(Path::to_path_buf)
}

#[cfg(feature = "native-dialogs")]
pub fn open_file() -> Task<Option<PathBuf>> {
    run_dialog(
        || {
            rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_open_archive())
                .add_filter(t::file_dialog_filter_img(), &["img", "dir"])
        },
        |dialog| async move {
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_file() -> Task<Option<PathBuf>> {
    Task::none()
}

/// Pick an entry-list manifest for archive comparison. The remembered
/// directory is only used when it still exists; rfd otherwise falls back to
/// its normal platform location.
#[cfg(feature = "native-dialogs")]
pub fn open_compare_manifest(directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    run_dialog(
        move || {
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_compare())
                .add_filter(
                    t::file_dialog_filter_entry_list(),
                    &["img.compare", "compare"],
                );
            apply_existing_directory(dialog, directory)
        },
        |dialog| async move {
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_compare_manifest(_directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    Task::none()
}

/// Pick a loose Bully animation group (`Anim/*.agr`). The model still comes
/// from the open archive; only the animation is read from disk.
#[cfg(feature = "native-dialogs")]
pub fn open_agr_file(directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    run_dialog(
        move || {
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_open_agr())
                .add_filter(t::file_dialog_filter_agr(), &["agr"]);
            apply_existing_directory(dialog, directory)
        },
        |dialog| async move {
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn open_agr_file(_directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(feature = "native-dialogs")]
pub fn import_files() -> Task<Vec<PathBuf>> {
    run_dialog(
        || {
            rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_import_files())
                .add_filter(
                    t::file_dialog_filter_importable(),
                    &["dff", "txd", "col", "ifp", "ipl", "ide", "dat"],
                )
        },
        |dialog| async move {
            dialog
                .pick_files()
                .await
                .map(|handles| {
                    handles
                        .into_iter()
                        .map(|handle| handle.path().to_path_buf())
                        .collect()
                })
                .unwrap_or_default()
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn import_files() -> Task<Vec<PathBuf>> {
    Task::none()
}

#[cfg(feature = "native-dialogs")]
pub fn import_folder() -> Task<Option<PathBuf>> {
    run_dialog(
        || rfd::AsyncFileDialog::new().set_title(t::file_dialog_import_folder()),
        |dialog| async move {
            dialog
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn import_folder() -> Task<Option<PathBuf>> {
    Task::none()
}

/// Pick the game folder used to resolve an archive's companion files,
/// starting at the folder currently in use.
#[cfg(feature = "native-dialogs")]
pub fn pick_game_folder(current: Option<PathBuf>) -> Task<Option<PathBuf>> {
    run_dialog(
        move || {
            let dialog = rfd::AsyncFileDialog::new().set_title(t::file_dialog_game_folder());
            apply_existing_directory(dialog, current)
        },
        |dialog| async move {
            dialog
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn pick_game_folder(_current: Option<PathBuf>) -> Task<Option<PathBuf>> {
    Task::none()
}

/// Pick a source image for texture replacement / TXD authoring.
#[cfg(feature = "native-dialogs")]
pub fn pick_image_file() -> Task<Option<PathBuf>> {
    run_dialog(
        || {
            rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_choose_image())
                .add_filter(
                    t::file_dialog_filter_images(),
                    &["png", "dds", "bmp", "tga"],
                )
        },
        |dialog| async move {
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn pick_image_file() -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(feature = "native-dialogs")]
pub fn save_archive(default_path: PathBuf, version: ImgVersion) -> Task<Option<SaveArchiveChoice>> {
    let directory = existing_parent_dir(&default_path);
    let file_name = default_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.img")
        .to_string();
    run_dialog(
        move || {
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_save_archive())
                .add_filter(t::file_dialog_filter_img(), &["img"])
                .set_file_name(file_name);
            apply_existing_directory(dialog, directory)
        },
        move |dialog| async move {
            dialog.save_file().await.map(|handle| SaveArchiveChoice {
                path: handle.path().to_path_buf(),
                version,
            })
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_archive(
    _default_path: PathBuf,
    _version: ImgVersion,
) -> Task<Option<SaveArchiveChoice>> {
    Task::none()
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
    run_dialog(
        move || {
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(t::file_dialog_export_list())
                .add_filter(
                    t::file_dialog_filter_entry_list(),
                    &["img.compare", "compare"],
                )
                .set_file_name(file_name);
            apply_existing_directory(dialog, directory)
        },
        |dialog| async move {
            dialog
                .save_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_compare_manifest(
    _default_path: PathBuf,
    _directory: Option<PathBuf>,
) -> Task<Option<PathBuf>> {
    Task::none()
}

/// Pick a destination folder for exporting entries or textures, starting at
/// `directory` when it still exists.
#[cfg(feature = "native-dialogs")]
pub fn save_folder(directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    run_dialog(
        move || {
            let dialog = rfd::AsyncFileDialog::new().set_title(t::file_dialog_export_folder());
            apply_existing_directory(dialog, directory)
        },
        |dialog| async move {
            dialog
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        },
    )
}

#[cfg(not(feature = "native-dialogs"))]
pub fn save_folder(_directory: Option<PathBuf>) -> Task<Option<PathBuf>> {
    Task::none()
}

#[cfg(all(test, feature = "native-dialogs"))]
mod tests {
    use super::*;

    #[test]
    fn native_dialog_task_requests_parent_window_before_spawning() {
        use iced::futures::StreamExt;

        let mut stream = iced_runtime::task::into_stream(open_file()).expect("dialog task stream");
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let first_action = rt.block_on(stream.next());
        assert!(
            matches!(
                first_action,
                Some(iced_runtime::Action::Window(
                    iced_runtime::window::Action::GetLatest(_)
                ))
            ),
            "native dialogs must query the active window for set_parent before opening"
        );
    }

    #[test]
    fn existing_parent_dir_ignores_missing_and_relative_file_names() {
        assert!(existing_parent_dir(Path::new("archive.img")).is_none());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("gta3.img");
        assert_eq!(existing_parent_dir(&file), Some(dir.path().to_path_buf()));
    }
}
