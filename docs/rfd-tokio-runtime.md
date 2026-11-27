# Native dialogs and async runtime: `rfd` 0.18.0 + `tokio` 1.54.1

Decision and architecture record for the joint `rfd 0.15.4` → `0.18.0` and
`tokio 1.52.3` → `1.54.1` upgrade pass (2026-11-27). Companion to
[allocator-rusty-alloc.md](allocator-rusty-alloc.md),
[hashmap-crate.md](hashmap-crate.md),
[rayon-parallelism.md](rayon-parallelism.md), and
[memmap2-io.md](memmap2-io.md).

## Where `rfd`, `tokio`, and Iced 0.14 overlap (and where they deliberately do not)

Reviewing `rfd 0.18.0`, `tokio 1.54.1`, and `iced_futures 0.14.0` together
clarified three boundaries that were previously blurred in `Cargo.toml` and
`src/`:

1. **`rfd` never used Tokio on Windows — and removed its `tokio` feature in 0.18.0.**
   - In `rfd 0.15.4`, `features = ["tokio"]` only forwarded to `ashpd?/tokio`
     (Linux XDG Desktop Portal). In `rfd 0.18.0`, the Linux portal backend
     replaced `ashpd` with direct `libdbus` + `zenity`, dropping the `tokio`
     feature and pruning `rfd`'s `ashpd` edge along with `mio` and `socket2`
     from `Cargo.lock` (`ashpd 0.10.3` remains solely through `dark-light` on
     Linux).
   - On Windows (`win_cid`), `rfd::AsyncFileDialog` deliberately spawns a
     dedicated short-lived OS thread (`std::thread::spawn`), calls
     `CoInitializeEx(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)`,
     invokes `IFileDialog::Show(hwndOwner)`, and signals a single-lock
     `rfd::oneshot` future awaited by Iced's Tokio executor.
   - **Why dialogs must stay on `rfd::AsyncFileDialog` rather than
     `tokio::task::spawn_blocking`:** Tokio's blocking pool reuses worker
     threads whose COM apartment state is not guaranteed to be clean STA
     (`RPC_E_CHANGED_MODE` causes `init_com` to fail silently and return
     `None`), and an open modal dialog would hold a pooled worker thread
     indefinitely.
2. **Iced 0.14 already owns the app's Tokio runtime (`rt`, `rt-multi-thread`, `time`).**
   - `iced_futures::backend::native::tokio::Executor` is
     `tokio::runtime::Runtime::new()`, and Iced enables `tokio` features
     `rt`, `rt-multi-thread`, and `time`.
   - Previously, `src/runtime.rs` spun up a **second** permanent global
     multi-threaded Tokio runtime (`worker_threads(2)`, `"imgeditor-tokio"`)
     whose only caller was `src/updater.rs::check_updates_future` — even though
     that future already ran inside Iced's Tokio runtime.
   - Previously, `Cargo.toml` enabled `tokio`'s `sync` feature solely for
     `viewer3d.rs`'s `mpsc::unbounded_channel` while omitting `time`, even
     though `src/ui/app.rs` called `tokio::time::sleep` and `enable_time()`
     directly (relying on `iced_futures` enabling `time` transitively).
3. **`rfd 0.18` + `iced::window::run` close the Windows modality / Always-on-Top gap.**
   - `rfd 0.17+` dropped the `Sized` bound on
     `set_parent<W: HasWindowHandle + HasDisplayHandle + ?Sized>(&W)`.
   - `iced::window::run(id, |window| ...)` provides `&dyn iced::window::Window`,
     which implements both `HasWindowHandle` and `HasDisplayHandle` using the
     same `raw-window-handle 0.6` crate as `rfd 0.18`.
   - Previously, all 10 native dialogs in `src/ui/dialogs.rs` ran unparented
     (`hwndOwner = NULL`). With **View ▸ Always on Top** (`WS_EX_TOPMOST`)
     active, an unowned Windows `IFileDialog` opens behind the topmost app
     window; even in normal mode, clicking IMG Editor buries the dialog and
     allows launching duplicate concurrent pickers.

## What changed in the crates

### `rfd` `0.15.4` → `0.18.0`
- **Windows async race fix:** Replaced the double-mutex `ThreadFuture`
  (which spawned eagerly on construction, held the result mutex across the
  entire `IFileDialog::Show` call, and had a `try_lock`/waker lost-wakeup
  window) with a single-lock `rfd::oneshot` channel spawned lazily on poll.
- **Unsized parent handles:** `set_parent` now accepts `&dyn Window` directly
  (`HasWindowHandle + HasDisplayHandle + ?Sized`).
- **Windows bindings:** Bumped `windows-sys` to `0.61` (matching our existing
  lockfile).
- **Hidden files toggle:** Added `set_show_hidden_files(bool)`
  (`FOS_FORCESHOWHIDDEN` on Windows); intentionally left at OS default so
  dialogs honor Explorer's user preference rather than cluttering file lists
  with hidden files.
- **Feature cleanup:** Dropped removed `tokio` feature and unused
  `common-controls-v6` (which only gates `TaskDialogIndirect` inside
  `rfd::MessageDialog`; IMGEditor uses Iced modals and has no ComCtl32 v6
  manifest). Kept default features (`xdg-portal`, `wayland`), which activate
  only Linux-target dependencies and add zero Windows crates while keeping
  non-Windows builds compatible with `rfd`'s `build.rs`.

### `tokio` `1.52.3` → `1.54.1`
- **Manifest floor & features:** `tokio = { version = "1.54", features = ["rt-multi-thread", "time"] }`
  — explicitly declares `time`, removes unused `sync`, and matches
  `iced_futures`'s Tokio feature set 1-to-1 (zero extra Tokio modules compiled).
- **Upstream runtime improvements (1.52.4–1.54.1):**
  - Cache-padded shared injection queue and idle state (`#8414`).
  - Monomorphization-time task-spawn auto-boxing decisions (`#8622`).
  - Runtime driver wakeup fix when `before_park` schedules work (`#8222`).
  - Reverted timer handle removal regression in `1.54.1` (`#8692`).

## Codebase changes

- **`Cargo.toml` / `Cargo.lock`:**
  - `tokio` `1.52.3` → `1.54.1` (`features = ["rt-multi-thread", "time"]`).
  - `rfd` `0.15.4` → `0.18.0` (default features, optional behind `native-dialogs`).
  - Removed `rfd`'s `ashpd` dependency along with `mio` and `socket2` from
    `Cargo.lock`.
- **`src/runtime.rs`:**
  - `runtime::spawn_blocking` now checks `tokio::runtime::Handle::try_current()`
    first and runs directly on Iced's ambient Tokio runtime whenever entered,
    falling back to `FALLBACK_RUNTIME` only for standalone/CLI callers outside
    Tokio. Eliminates the redundant 2-thread `"imgeditor-tokio"` runtime that
    previously lived for the whole GUI session after startup update checks.
  - Added unit test `spawn_blocking_reuses_ambient_tokio_runtime_without_building_fallback`.
- **`src/inspector/viewer3d.rs` & `src/ui/app.rs`:**
  - Replaced `spawn_render_window` / `spawn_dff_render_window` /
    `spawn_col_render_window` (raw `std::thread::Builder`, manual
    `panic::catch_unwind`, `tokio::sync::mpsc::unbounded_channel`, and
    `App::viewer_rxs` polled every 250 ms via `poll_viewer_rxs`) with direct
    synchronous helpers (`open_nif_external_viewer`, `open_dff_external_viewer`,
    `open_col_external_viewer`) taking `&[u8]` slices.
  - `EntryAction::RenderExternal` now reads entry bytes off `source_mmap` and
    exports/launches inside `tokio::task::spawn_blocking`, delivering
    `Message::ExternalViewerFinished(ViewerEvent)` immediately on completion
    (and catching panics via `JoinError`). Also fixed COL preview filename
    stem extraction to use `Path::file_stem()`.
- **`src/ui/app.rs` (idle Tokio timer elimination):**
  - `App::subscription` previously ran two `iced::time::every` Tokio interval
    timers unconditionally (`TickProgress` every 250 ms and `DebounceTick`
    every 150 ms — ~10.7 wakeups/sec forever even while idle).
  - Gated both behind `App::progress_tick_active()` (active progress, toast,
    toast pulse, or running animation) and `App::debounce_tick_active()`
    (`filter_pending`), dropping idle Tokio timer wakeups to **0 Hz** on an
    idle open archive.
  - Added unit tests `idle_tokio_timers_stay_dormant_until_state_needs_them`
    and `external_viewer_finished_logs_and_toasts_without_polling`.
- **`src/ui/dialogs.rs` & `src/ui/app.rs`:**
  - Routed all 10 native file/folder dialogs through one helper (`run_dialog`)
    that queries `iced::window::latest()`, binds the parent window via
    `iced::window::run(id, |window| dialog.set_parent(window))`, and awaits
    the `rfd::AsyncFileDialog` future on Iced's Tokio runtime (falling back
    cleanly without a parent if no window is open).
  - Added remembered starting directories (`set_directory` when existing) to
    `save_archive` (archive's parent folder), `save_folder` (selected archive
    or config `last_export_folder`), and `open_agr_file` (`<game_root>/Anim`
    or `<game_root>`).
  - Added unit tests `native_dialog_task_requests_parent_window_before_spawning`
    and `existing_parent_dir_ignores_missing_and_relative_file_names`.
