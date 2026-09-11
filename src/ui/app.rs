use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use quick_cache::sync::GuardResult;

use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, scroll_to};
use iced::keyboard::{Event as KeyboardEvent, Modifiers};
use iced::widget::{Space, container, pane_grid};
use iced::{Element, Point, Subscription, Task, Theme};
use iced_aw::menu::{Item, Menu, MenuBar};
use iced_fonts::LUCIDE_FONT_BYTES;

use crate::archive::{ArchiveInfo, ExportStatus, SortColumn};
use crate::dev_logger;
use crate::sort::{SortChain, SortDirection, SortKey, SortPriority};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Config, ThemeMode};
use crate::editor::Editor;
use crate::inspector::scene3d::mesh::SceneTexture;
use crate::inspector::viewer3d::{self, ViewerEvent};
use crate::parser::{
    DecodedTexture, EntryInspection, ImgVersion, inspect_entry_cached,
};
use crate::tasks::{
    ExportMode, ExportTask, FolderDuplicatePolicy, FolderImportOutcome, FolderImportPlan,
    FolderImportSummary, FolderImportTask, PackOutcome, PackTask, SaveTask, scan_import_folder,
};
use crate::ui::animator::Animator;
use crate::ui::design::Design;
use crate::ui::dialogs::{self, SaveArchiveChoice};
use crate::ui::fonts;
use crate::ui::icons;
use crate::ui::keymap::{Shortcut, detect_pressed, shortcut_display};
use crate::ui::theme::resolve_theme;
use crate::ui::tokens::motion::DurationPreset;
use crate::ui::widgets as w;
use crate::updater::{UpdateResult, UpdateState, check_updates_future};

const REPO_URL: &str = "https://github.com/CloudyTabzy/IMGEditor-Plus";
const UPDATER_REPO: &str = "CloudyTabzy/IMGEditor-Plus";
const SEARCH_INPUT_ID: &str = "search_input";
/// Rows shown in the search prediction dropdown.
const MAX_SEARCH_PREDICTIONS: usize = 8;
const RENAME_INPUT_ID: &str = "rename_input";

fn is_renderable_model_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".nif") || lower.ends_with(".dff")
}

pub const ANIM_PROGRESS: crate::ui::animator::AnimationId = 1;
pub const ANIM_TOAST_OPACITY: crate::ui::animator::AnimationId = 2;
pub const ANIM_ENTRY_FEEDBACK: crate::ui::animator::AnimationId = 3;
pub const ANIM_ARCHIVE_TAB_FEEDBACK: crate::ui::animator::AnimationId = 4;
pub const ANIM_INSPECTOR_TAB_FEEDBACK: crate::ui::animator::AnimationId = 5;
pub const ANIM_CLICK_RIPPLE: crate::ui::animator::AnimationId = 6;
pub const ANIM_TOAST_REVEAL: crate::ui::animator::AnimationId = 7;
/// Quit fade: covers the window as it closes so no teardown frame
/// (swapchain/DWM artifacts) can flash at the user.
pub const ANIM_QUIT_FADE: crate::ui::animator::AnimationId = 8;
/// Give the native window a short event-loop turn to become hidden before
/// Iced releases the wgpu surface. Windows can otherwise expose a default
/// compositor frame while the HWND is being destroyed.
const QUIT_HIDE_SETTLE: Duration = Duration::from_millis(120);
/// Keep the exit transition quick while leaving enough time for a final
/// opaque frame to be presented before the native hide.
const QUIT_FADE_DURATION: Duration = Duration::from_millis(120);
const QUIT_FADE_DEADLINE: Duration = Duration::from_millis(180);

#[derive(Debug, Clone)]
pub enum OpenArchiveOutcome {
    Opened(Box<ArchiveInfo>),
    Unsupported,
    Failed(String),
}

pub const ABOUT_TEXT: &str = concat!(
    "IMG Editor Plus v",
    env!("CARGO_PKG_VERSION"),
    "\n\nA pure Rust desktop editor for GTA IMG archives.\n\n",
    "Made by CloudyTabzy & Agents\n",
    "Based on the original ",
    "IMG Editor by Grinch_\n",
    "(https://github.com/user-grinch/IMGEditor)\n\n",
    "Supported formats:\n",
    "- GTA III\n",
    "- GTA Vice City\n",
    "- GTA San Andreas\n",
    "- Bully Scholarship Edition"
);

/// Optional inertia layered after Iced's native autoscroll settles in its
/// neutral zone. It deliberately stores one sampled velocity rather than an
/// accumulating multiplier, so a long hold at the screen edge cannot run away.
#[derive(Debug, Clone, Copy, Default)]
struct AutoScrollMomentum {
    origin: Option<Point>,
    last_live_velocity: Option<f32>,
    tail: Option<AutoScrollMomentumTail>,
}

#[derive(Debug, Clone, Copy)]
struct AutoScrollMomentumTail {
    velocity: f32,
    remaining_distance: f32,
}

impl AutoScrollMomentum {
    // These match Iced 0.14's native autoscroll curve so the tail begins only
    // after the native controller has reached its own neutral zone.
    const DEAD_ZONE: f32 = 20.0;
    const SMOOTHNESS: f32 = 1.5;
    const MIN_SOURCE_SPEED: f32 = 750.0;
    const MAX_SOURCE_SPEED: f32 = 2_400.0;
    const INITIAL_SPEED_FRACTION: f32 = 0.28;
    const MAX_INITIAL_SPEED: f32 = 672.0;
    const DAMPING_PER_SECOND: f32 = 8.5;
    const MIN_TAIL_SPEED: f32 = 18.0;
    const MAX_TAIL_DISTANCE: f32 = 96.0;

    fn begin(&mut self, origin: Option<Point>) {
        self.origin = origin;
        self.last_live_velocity = None;
        self.tail = None;
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn clear_tail(&mut self) {
        self.last_live_velocity = None;
        self.tail = None;
    }

    fn is_active(&self) -> bool {
        self.tail.is_some()
    }

    fn native_velocity_at(&self, position: Point) -> Option<f32> {
        let origin = self.origin?;
        let delta = position.y - origin.y;
        if delta.abs() < Self::DEAD_ZONE {
            return Some(0.0);
        }

        Some(
            delta.signum()
                * delta
                    .abs()
                    .powf(Self::SMOOTHNESS)
                    .min(Self::MAX_SOURCE_SPEED),
        )
    }

    /// Returns true when a tail was armed by re-entering the neutral zone.
    fn update_pointer(&mut self, position: Point, enabled: bool) -> bool {
        let Some(velocity) = self.native_velocity_at(position) else {
            return false;
        };

        if velocity != 0.0 {
            self.last_live_velocity = Some(velocity);
            self.tail = None;
            return false;
        }

        let Some(live_velocity) = self.last_live_velocity.take() else {
            return false;
        };
        if !enabled || !live_velocity.is_finite() || live_velocity.abs() < Self::MIN_SOURCE_SPEED {
            self.tail = None;
            return false;
        }

        let initial_speed = (live_velocity.abs() * Self::INITIAL_SPEED_FRACTION)
            .min(Self::MAX_INITIAL_SPEED);
        let remaining_distance = (initial_speed / Self::DAMPING_PER_SECOND)
            .min(Self::MAX_TAIL_DISTANCE);
        self.tail = Some(AutoScrollMomentumTail {
            velocity: live_velocity.signum() * initial_speed,
            remaining_distance,
        });
        true
    }

    /// Advance the exponential decay analytically, keeping it frame-rate
    /// independent even if the UI misses a frame.
    fn advance(&mut self, dt: Duration) -> Option<f32> {
        let tail = self.tail.as_mut()?;
        let seconds = dt.as_secs_f32();
        if seconds <= 0.0 {
            return None;
        }

        let decay = (-Self::DAMPING_PER_SECOND * seconds).exp();
        let unconstrained = tail.velocity * (1.0 - decay) / Self::DAMPING_PER_SECOND;
        if !unconstrained.is_finite() {
            self.clear_tail();
            return None;
        }
        let delta = unconstrained.signum() * unconstrained.abs().min(tail.remaining_distance);
        tail.remaining_distance = (tail.remaining_distance - delta.abs()).max(0.0);
        tail.velocity *= decay;

        if tail.remaining_distance <= f32::EPSILON || tail.velocity.abs() < Self::MIN_TAIL_SPEED {
            self.tail = None;
        }

        Some(delta)
    }
}

/// The specific scene currently being decoded off the UI thread.
///
/// Keeping the identity with the loading state prevents a late completion for
/// an older selection from replacing the current loading transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewerLoadState {
    target: (usize, usize),
    entry_name: String,
}

/// Application event type. Heterogeneous by design — some variants carry
/// large payloads (`Viewer3dLoadCompleted::Scene`, `ExportCompleted::Vec<String>`)
/// while most are unit or single-value. Boxing the large variants would
/// shrink the inline footprint but force a heap allocation on every
/// `iced::Task::done(Message::…)`, which is the event-loop hot path.
/// Tracked in TODO §6.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    ShortcutPressed(Shortcut),

    NewArchive,
    OpenArchive,
    OpenArchiveResult(Option<PathBuf>),
    ArchiveOpenCompleted {
        path: PathBuf,
        outcome: OpenArchiveOutcome,
    },
    /// Open a path from the recent-files list. Carries the raw
    /// path as it appeared in the menu; missing entries are
    /// filtered out before this fires.
    OpenRecent(PathBuf),
    SaveArchive,
    SaveArchiveAs,
    SaveArchiveAsResult(Option<SaveArchiveChoice>),
    SaveCompleted {
        index: usize,
        result: Result<ArchiveInfo, String>,
    },
    PackArchive,
    PackCompleted {
        index: usize,
        result: Result<PackOutcome, String>,
    },
    CloseSelectedArchive,
    CloseArchiveTab(usize),
    SelectArchiveTab(usize),

    ImportFiles,
    ImportFilesResult(Vec<PathBuf>),
    /// Pre-flight check finished for a candidate import.
    ImportPreflightCompleted {
        index: usize,
        paths: Vec<PathBuf>,
        folder: Option<Box<(crate::tasks::FolderImportPlan, crate::tasks::FolderDuplicatePolicy)>>,
        checks: Vec<crate::compat::scan::ImportFileCheck>,
    },
    /// User chose to import despite flagged formats.
    ImportCheckConfirmed,
    /// User cancelled the import at the pre-flight dialog.
    ImportCheckCancelled,
    ImportCompleted {
        index: usize,
        count: usize,
        /// Whether the pre-flight format check ran (a target was set).
        checked: bool,
        result: Result<ArchiveInfo, String>,
    },
    ImportFolder,
    ImportFolderResult(Option<PathBuf>),
    FolderScanCompleted {
        index: usize,
        result: Result<FolderImportPlan, String>,
    },
    ConfirmFolderImport(FolderDuplicatePolicy),
    CancelFolderImport,
    FolderImportCompleted {
        index: usize,
        result: Result<FolderImportOutcome, String>,
    },
    ExportAll,
    ExportSelected,
    ExportFolderResult(Option<PathBuf>),
    ExportCompleted {
        index: usize,
        result: Result<(usize, Vec<String>), String>,
    },

    SelectAll,
    InvertSelection,
    ClearSelection,
    DeleteSelected,
    StartRename,
    RenameInputChanged(String),
    CommitRename,
    CancelRename,
    CancelActive,

    SearchChanged(String),
    /// Clear the search box, reset the filter, and refocus the input.
    ClearSearch,
    SearchPredictMove(i32),
    SearchPredictCommit,
    SearchPredictDismiss,
    SearchPredictPick(usize),
    SearchPickDidYouMean,
    /// A left click that no widget captured: dismiss the search
    /// prediction dropdown if it is open.
    UncapturedPress,
    /// Click on the search strip's label area: focus the input and keep
    /// the prediction dropdown alive.
    FocusSearchInput,
    SearchFocusChanged(bool),
    RenameFocusChanged(bool),
    DebounceTick,
    RefreshFilter,

    CopySelectedEntryDetails,
    CopyLogs,

    EntryClicked(usize),
    EntryDoubleClicked(usize),
    EntryRightClicked(usize),
    EntryContextAction(EntryAction),
    HideContextMenu,
    ModifiersChanged(Modifiers),
    PointerMoved(Point),
    EntryTableHoverChanged(bool),
    AnimationTick(std::time::Instant),
    AutoScrollStarted,
    AutoScrollEnded,
    /// Escape ends autoscroll and dismisses the search prediction dropdown.
    AutoScrollEscape,

    ShowAbout,
    HideAbout,
    ShowWelcome,
    HideWelcome,
    ToggleWelcomePersist(bool),
    ToggleUpdateDisabled(bool),
    ToggleUpdateNotifyDisabled(bool),
    ShowUnsupported(PathBuf),
    HideUnsupported,
    VisitRepository,
    HideUpdateStatus,

    CheckUpdatesManual,
    UpdateResultReceived(UpdateResult),

    SetTheme(ThemeMode),
    ToastTimeout,
    TickProgress,
    PaneResized(pane_grid::ResizeEvent),
    OpenLastExportFolder,
    SortBy(SortColumn),
    ScrollOffsetChanged {
        y: f32,
        max_y: f32,
    },

    FilesDropped(PathBuf),

    /// Toolbar "Validate textures": profile every TXD in the selected
    /// archive against the per-game compatibility tables.
    /// Opens the game-picker popup for texture validation.
    OpenValidatorPopup,
    /// Closes the validator popup without running anything.
    CloseValidatorPopup,
    /// Runs the texture validator against one game profile.
    ValidateArchiveFor(&'static str),
    /// Toggles the validator's entry-row tinting.
    SetCompatHighlight(bool),
    /// Grab the tab-width divider.
    TabResizeStarted,
    /// Cursor x while dragging the divider.
    TabResizeMoved(f32),
    /// Release the tab-width drag; persists the width.
    TabResizeEnded,
    /// User acknowledged the pre-save report and wants to write.
    SaveCheckConfirmed,
    /// Toggle the dialog's lossless header-repair checkbox.
    SaveCheckFixToggled(bool),
    /// User cancelled at the pre-save report.
    SaveCheckCancelled,
    /// Replace the selected texture (opens the image picker).
    TextureReplaceRequested,
    /// Image picker came back for a replacement. `attempt` invalidates
    /// results from a superseded picker or re-plan.
    ReplaceImagePicked {
        attempt: u64,
        path: Option<PathBuf>,
    },
    /// Background planning finished for a replacement.
    ReplacePlanned {
        attempt: u64,
        result: Box<Result<ReplacePlanReady, String>>,
    },
    /// The dialog's format pick changed; re-plan.
    ReplaceFormatChanged(crate::compat::encode::EncodeFormat),
    /// Toggle high-quality DXT for the replacement; re-plan.
    ReplaceHighQualityToggled(bool),
    /// Background re-plan finished.
    ReplacePlanRefreshed {
        attempt: u64,
        result: Box<Result<ReplacePlanReady, String>>,
    },
    /// Apply the replacement.
    ReplaceConfirmed,
    /// Background replacement finished.
    ReplaceApplied {
        archive_index: usize,
        entry_index: usize,
        result: Result<AppliedBytes, String>,
    },
    /// Dismiss the replace dialog.
    ReplaceCancelled,
    /// Author a new TXD from an image (opens the image picker).
    ImportImageAsTxdRequested,
    /// Image picker came back for TXD authoring. `attempt` invalidates
    /// results from a superseded picker or re-plan.
    NewTxdImagePicked {
        attempt: u64,
        path: Option<PathBuf>,
    },
    /// Background planning finished for a new TXD.
    NewTxdPlanned {
        attempt: u64,
        result: Box<Result<NewTxdPlanReady, String>>,
    },
    /// The dialog's name field changed.
    NewTxdNameChanged(String),
    /// The dialog's format pick changed; re-plan.
    NewTxdFormatChanged(crate::compat::encode::EncodeFormat),
    /// Toggle high-quality DXT for the new TXD; re-plan.
    NewTxdHighQualityToggled(bool),
    /// Author the new TXD entry.
    NewTxdConfirmed,
    /// Dismiss the new-TXD dialog.
    NewTxdCancelled,
    /// Convert every selected TXD texture to the target dialect.
    BulkConvertRequested,
    /// Background bulk planning finished.
    BulkConvertPlanned(Box<Result<BulkPlanReady, String>>),
    /// Apply the bulk conversion.
    BulkConvertConfirmed,
    /// Background bulk conversion finished.
    BulkConvertApplied {
        archive_index: usize,
        result: Result<SavePatches, String>,
    },
    /// Dismiss the bulk-convert dialog.
    BulkConvertCancelled,
    /// Background header repair finished; apply the patches and save.
    SaveFixesReady {
        index: usize,
        patches: SavePatches,
        path: PathBuf,
        version: crate::parser::ImgVersion,
        remove_existing: bool,
    },
    /// Window close button pressed; may open the unsaved-changes guard.
    WindowCloseRequested(iced::window::Id),
    /// Save from the unsaved-changes guard (then close).
    CloseGuardSave,
    /// Discard from the unsaved-changes guard.
    CloseGuardDiscard,
    /// Cancel from the unsaved-changes guard.
    CloseGuardCancel,
    /// The quit fade finished (or its deadline passed); close the window.
    QuitFadeDone,
    /// The native window has been hidden; it is now safe to release Iced's
    /// surface and destroy the window.
    QuitWindowHidden,
    /// Content probe finished; the hint is advisory only.
    TargetProbed {
        archive_index: usize,
        hint: Option<crate::compat::hint::TargetHint>,
    },
    CompatibilityValidated {
        archive_index: usize,
        result: Result<crate::compat::scan::ScanReport, String>,
    },

    TextureDecodeRequested,
    TextureDecoded {
        archive_index: usize,
        index: usize,
        result: Result<Arc<Vec<DecodedTexture>>, String>,
    },
    TextureSelect(usize),
    TextureExport,
    TextureExportFolderResult(Option<PathBuf>),
    TextureUvToggled(bool),
    ViewTextureGridToggled(bool),
    ViewTextureGridSize(u32),
    SetNavigationGizmoVisible(bool),
    ToggleAutoscrollMomentum(bool),
    ToggleMotionEffects(bool),
    ToggleSelectionPulse(bool),
    ToggleClickRipple(bool),
    ToggleIconMicroMotion(bool),
    ToggleSearchBar(bool),
    ToggleLiteralFileTypes(bool),
    ToggleContextAccumulate(bool),

    ExportEmbeddedTexturesRequest {
        entry_index: usize,
        nif_basename: String,
    },
    ExportEmbeddedTexturesFolderResult {
        entry_index: usize,
        nif_basename: String,
        folder: Option<PathBuf>,
    },
    ExportEmbeddedTexturesCompleted {
        entry_index: usize,
        nif_basename: String,
        result: Result<crate::inspector::texture_export::ExportReport, String>,
    },

    Viewer3dRequestLoad {
        archive_index: usize,
        entry_index: usize,
    },
    Viewer3dLoadSelected,
    Viewer3dLoadCompleted {
        archive_index: usize,
        entry_index: usize,
        /// The archive generation at request time. A completion from an
        /// older generation (entries mutated while the load ran) must not
        /// resolve onto the new data at the same entry index.
        generation: u64,
        result: Result<Arc<crate::inspector::scene3d::Scene>, String>,
        /// An `IdeMap` freshly built by the load task, so the app can
        /// memoize it per game root. `None` when a cached map was reused or
        /// the archive has no game root.
        ide_map: Option<BuiltIdeMap>,
    },
    Viewer3dSelectTab(InspectorTab),
    Viewer3dClear,
    Viewer3dReset,
    Viewer3dToggleCenterOrigin,
    Viewer3dToggleWireframe,
    Viewer3dToggleGrid,
    Viewer3dToggleCullBackfaces,
    Viewer3dToggleTextured,
    Viewer3dToggleAlphaBlend,

    // Sort Manager dialog. The dialog edits a draft copy of the
    // active archive's SortChain; "Apply" commits the draft to the
    // archive + the global default. SlotIndex is a NewType so the
    // compiler refuses accidental cross-pollination with other
    // numeric state in the handler.
    OpenSortManager,
    CloseSortManager,
    SortApplyDraft,
    SortResetDraft,
    SortAddSlot,
    SortRemoveSlot(SortSlotIndex),
    SortMoveSlotUp(SortSlotIndex),
    SortMoveSlotDown(SortSlotIndex),
    SortToggleSlotEnabled(SortSlotIndex),
    SortSetSlotKey(SortSlotIndex, crate::sort::SortKey),
    SortSetSlotDirection(SortSlotIndex, crate::sort::SortDirection),
    SortSelectPreset(SortPreset),

    // ---- Drag-and-drop between archives ----
    /// User started dragging selected entries from a source archive
    /// tab. The App's `drag_state` is updated to remember the source
    /// archive + the entry indices being moved.
    ArchiveDragStarted {
        source: usize,
    },
    /// Mouse moved while dragging. The optional `over` argument is
    /// the archive index the cursor is currently over (from
    /// `on_enter` / `on_exit` events on the tab strip). `None` means
    /// the cursor is over empty space.
    ArchiveDragMoved {
        over: Option<usize>,
    },
    /// User released the mouse. If `over` is set, the entries are
    /// moved from the source to that archive; otherwise the drag is
    /// cancelled. The source is in the App's drag_state, not in the
    /// message, to avoid passing it through every event.
    ArchiveDragReleased,
    /// Cancel the drag in progress (Escape pressed, focus lost,
    /// window close, etc.). Distinct from "released" because the
    /// latter implies an explicit drop target.
    ArchiveDragCancelled,
}

/// NewType around `usize` that names a slot inside the Sort Manager's
/// draft chain. Using a distinct type prevents the compiler from
/// accepting a row index where a slot index is expected (or vice
/// versa) — both are `usize` underneath, but the wrappers make the
/// call sites self-documenting and catch bugs at the type level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SortSlotIndex(pub usize);

/// Built-in sort presets the user can apply with one click. The
/// index matches the dropdown order in `view.rs`; the payload is
/// the chain shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortPreset {
    NameAZ,
    NameZA,
    TypeThenName,
    SizeDesc,
    OffsetAsc,
}

impl SortPreset {
    /// Convert this preset to a `SortChain`. Each preset is a
    /// single fixed chain the user can further edit (toggle
    /// directions, add tiebreakers, etc.) before applying.
    pub fn to_chain(self) -> crate::sort::SortChain {
        use crate::sort::{SortChain, SortDirection, SortKey, SortPriority};
        let p = |key, dir| SortPriority {
            enabled: true,
            key,
            direction: dir,
        };
        match self {
            SortPreset::NameAZ => SortChain::new(vec![p(SortKey::Name, SortDirection::Ascending)]),
            SortPreset::NameZA => SortChain::new(vec![p(SortKey::Name, SortDirection::Descending)]),
            SortPreset::TypeThenName => SortChain::new(vec![
                p(SortKey::Type, SortDirection::Ascending),
                p(SortKey::Name, SortDirection::Ascending),
            ]),
            SortPreset::SizeDesc => {
                SortChain::new(vec![p(SortKey::Size, SortDirection::Descending)])
            }
            SortPreset::OffsetAsc => {
                SortChain::new(vec![p(SortKey::Offset, SortDirection::Ascending)])
            }
        }
    }

    /// Display name for the dropdown. Kept here (not in `view.rs`)
    /// so the preset list reads top-to-bottom in one place.
    pub fn display_name(self) -> &'static str {
        match self {
            SortPreset::NameAZ => "Name (A→Z)",
            SortPreset::NameZA => "Name (Z→A)",
            SortPreset::TypeThenName => "Type, then name",
            SortPreset::SizeDesc => "Size (big → small)",
            SortPreset::OffsetAsc => "Offset (low → high)",
        }
    }

    /// All presets in picker order. Used by the dropdown to
    /// populate its list without hard-coding it in `view.rs`.
    pub const ALL: &'static [SortPreset] = &[
        SortPreset::NameAZ,
        SortPreset::NameZA,
        SortPreset::TypeThenName,
        SortPreset::SizeDesc,
        SortPreset::OffsetAsc,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Export,
    Model3D,
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RippleTarget {
    Entry {
        archive_index: usize,
        entry_index: usize,
    },
    ArchiveTab(usize),
    InspectorTab(InspectorTab),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RippleState {
    target: RippleTarget,
    origin: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    CopyName,
    Rename,
    Delete,
    Export,
    Render,
    RenderExternal,
    ViewTextures,
    ExportEmbeddedTextures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Table,
    Info,
}

/// An import paused at the pre-flight format-check dialog because at
/// least one file carries a format the target engine cannot consume.
#[derive(Debug, Clone)]
pub struct PendingImport {    pub index: usize,
    pub paths: Vec<PathBuf>,
    /// Folder imports resume through their plan + chosen duplicate policy.
    pub folder: Option<(FolderImportPlan, FolderDuplicatePolicy)>,
    pub checks: Vec<crate::compat::scan::ImportFileCheck>,
}

impl PendingImport {
    /// Files that fail the target's dialect, worst verdict first.
    pub fn flagged(&self) -> Vec<&crate::compat::scan::ImportFileCheck> {
        let mut flagged: Vec<_> = self
            .checks
            .iter()
            .filter(|check| check.has_issues())
            .collect();
        flagged.sort_by_key(|check| std::cmp::Reverse(check.worst));
        flagged
    }
}

/// A save waiting on the pre-save report. `issue` is `None` while a
/// validation scan is still running; it is filled once the report lands,
/// and the save proceeds silently when nothing needs review.
#[derive(Debug, Clone)]
pub struct PendingSave {
    pub index: usize,
    pub path: PathBuf,
    pub version: crate::parser::ImgVersion,
    pub remove_existing: bool,
    pub issue: Option<crate::compat::save::SaveIssue>,
    /// The dialog's repair checkbox: patch fixable DXT headers before
    /// writing. Only meaningful when the issue has fixable reports.
    pub fix: bool,
}

/// Close or quit waiting on the unsaved-changes guard.
#[derive(Debug, Clone, Copy)]
pub enum PendingClose {
    /// Closing one archive tab (dirty).
    Archive(usize),
    /// The window close button, with at least one dirty archive open.
    Window(iced::window::Id),
}

/// Patched entry bytes from the background normalize pass, with a
/// compact `Debug` so message dumps never print megabytes of pixels.
#[derive(Clone)]
pub struct SavePatches(pub Vec<(usize, Arc<Vec<u8>>)>);

impl SavePatches {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for SavePatches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SavePatches({} entries)", self.0.len())
    }
}

/// A conversion plan with a compact `Debug`: plans carry preview
/// pixels, and message dumps must stay readable.
#[derive(Clone)]
pub struct CompactPlan(pub Arc<crate::compat::convert::ConversionPlan>);

impl std::fmt::Debug for CompactPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConversionPlan({} -> {})",
            self.0.source_label, self.0.format_label
        )
    }
}

/// Applied entry bytes with a compact `Debug`.
#[derive(Clone)]
pub struct AppliedBytes(pub Arc<Vec<u8>>);

impl std::fmt::Debug for AppliedBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AppliedBytes({} bytes)", self.0.len())
    }
}

/// Planning result for a texture replacement.
#[derive(Clone)]
pub struct ReplacePlanReady {
    pub archive_index: usize,
    pub entry_index: usize,
    pub texture_index: usize,
    pub source_path: PathBuf,
    pub source_name: String,
    pub texture_name: String,
    pub before: Option<(u32, u32, Arc<Vec<u8>>)>,
    pub plan: CompactPlan,
    pub entry: crate::archive::EntryInfo,
    pub archive_path: Option<PathBuf>,
    pub target: &'static crate::compat::games::GameProfile,
    pub archive_name: String,
}

impl std::fmt::Debug for ReplacePlanReady {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplacePlanReady")
            .field("archive_index", &self.archive_index)
            .field("entry_index", &self.entry_index)
            .field("texture_index", &self.texture_index)
            .field("source_name", &self.source_name)
            .field("texture_name", &self.texture_name)
            .field("plan", &self.plan)
            .finish()
    }
}

/// Open replace-dialog state.
pub struct ReplaceState {
    pub archive_index: usize,
    pub entry_index: usize,
    pub texture_index: usize,
    pub source_path: PathBuf,
    pub source_name: String,
    pub texture_name: String,
    /// The entry's source (archive or loose file) for re-reading bytes.
    pub entry: crate::archive::EntryInfo,
    pub archive_path: Option<PathBuf>,
    pub target: &'static crate::compat::games::GameProfile,
    pub archive_name: String,
    pub chooser: crate::compat::encode::EncodeFormat,
    /// High-quality DXT (iterative cluster fit) for this plan.
    pub high_quality: bool,
    pub plan: CompactPlan,
    pub before_handle: iced::widget::image::Handle,
    pub after_handle: iced::widget::image::Handle,
    pub planning: bool,
}

/// Planning result for a new TXD entry.
#[derive(Clone)]
pub struct NewTxdPlanReady {
    pub archive_index: usize,
    pub source_path: PathBuf,
    pub source_name: String,
    pub texture_name: String,
    pub target: &'static crate::compat::games::GameProfile,
    pub plan: CompactPlan,
}

impl std::fmt::Debug for NewTxdPlanReady {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewTxdPlanReady")
            .field("archive_index", &self.archive_index)
            .field("source_name", &self.source_name)
            .field("texture_name", &self.texture_name)
            .field("plan", &self.plan)
            .finish()
    }
}

/// Open new-TXD dialog state.
pub struct NewTxdState {
    pub archive_index: usize,
    pub source_path: PathBuf,
    pub source_name: String,
    pub texture_name: String,
    pub target: &'static crate::compat::games::GameProfile,
    pub chooser: crate::compat::encode::EncodeFormat,
    /// High-quality DXT (iterative cluster fit) for this plan.
    pub high_quality: bool,
    pub plan: CompactPlan,
    pub after_handle: iced::widget::image::Handle,
    pub planning: bool,
}

/// One entry's bulk-conversion plan.
#[derive(Clone)]
pub struct BulkEntryPlan {
    pub entry_index: usize,
    pub file_name: String,
    /// (texture index, texture name, plan) for textures needing work.
    pub textures: Vec<(usize, String, CompactPlan)>,
    pub skipped_native: usize,
    pub failed: usize,
}

impl std::fmt::Debug for BulkEntryPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BulkEntryPlan({} textures, {} skipped, {} failed)",
            self.textures.len(),
            self.skipped_native,
            self.failed
        )
    }
}

/// Bulk-conversion planning result.
#[derive(Clone)]
pub struct BulkPlanReady {
    pub archive_index: usize,
    pub source_label: String,
    pub entries: Vec<BulkEntryPlan>,
}

impl std::fmt::Debug for BulkPlanReady {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BulkPlanReady({} entries from {})",
            self.entries.len(),
            self.source_label
        )
    }
}

/// Open bulk-convert dialog state.
pub struct BulkConvertState {
    pub archive_index: usize,
    pub source_label: String,
    pub entries: Vec<BulkEntryPlan>,
}

pub struct App {
    pub editor: Editor,
    pub config: Config,
    pub search: String,
    pub rename_buffer: String,
    search_focused: bool,
    rename_focused: bool,
    /// Fuzzy-search prediction rows: `(entry index, display name)`,
    /// best match first. Only the top few are kept.
    pub(crate) search_predictions: Vec<(usize, String)>,
    /// Index into the virtual prediction list (`search_predictions`
    /// plus the optional did-you-mean row) highlighted by keyboard
    /// navigation.
    pub(crate) prediction_index: Option<usize>,
    /// "Did you mean …" suggestion: `(entry index, display name)`,
    /// populated when the query matches nothing but a Jaro-Winkler
    /// near-miss exists.
    pub(crate) did_you_mean: Option<(usize, String)>,
    /// Set when the user dismisses the prediction dropdown with
    /// Escape; reset on the next query change.
    predictions_dismissed: bool,
    pending_shortcut: Option<Shortcut>,
    pending_search_focus: Option<bool>,
    pending_rename_focus: Option<bool>,
    pub show_about: bool,
    pub show_welcome: bool,
    pub welcome_persist: bool,
    pub show_unsupported: Option<PathBuf>,
    pub show_update_status: Option<String>,
    pub update_state: UpdateState,
    pub update_check_manual: bool,
    pub toast: Option<String>,
    pub pending_folder_import: Option<(usize, FolderImportPlan)>,
    /// Import waiting on the pre-flight format check dialog.
    pub pending_import: Option<PendingImport>,
    /// Save waiting on the pre-save report dialog (or on a validation
    /// scan that has to finish first).
    pub pending_save: Option<PendingSave>,
    /// (archive, entry) waiting on the image picker for a replacement.
    pub replace_request: Option<(usize, usize)>,
    /// Attempt counter for the replace flow: background results from a
    /// superseded picker or re-plan are dropped instead of re-opening
    /// the dialog.
    pub replace_attempt: u64,
    /// Attempt counter for the new-TXD flow (same purpose).
    pub new_txd_attempt: u64,
    /// True while the new-TXD image picker is open (guards double
    /// opens from repeated toolbar clicks).
    pub new_txd_picker_open: bool,
    /// True while a new-TXD plan is being prepared in the background
    /// (no dialog exists yet; a second request would invalidate it).
    pub new_txd_plan_in_flight: bool,
    /// True while a replace plan is being prepared in the background.
    pub replace_plan_in_flight: bool,
    /// Open replace-texture dialog state (Phase B).
    pub pending_replace: Option<ReplaceState>,
    /// Open new-TXD dialog state (Phase B).
    pub pending_new_txd: Option<NewTxdState>,
    /// Open bulk-convert dialog state (Phase B).
    pub pending_bulk: Option<BulkConvertState>,
    /// Close/quit waiting on the unsaved-changes guard.
    pub pending_close: Option<PendingClose>,
    /// After a successful save of this archive index, close it (the
    /// unsaved-changes guard's "Save" path).
    pub close_after_save: Option<usize>,
    /// Window being faded out before closing.
    pub quitting: Option<iced::window::Id>,
    /// Working copy of the sort chain while the Sort Manager
    /// dialog is open. Edits land here first; "Apply" commits the
    /// draft to the live archive + config. `None` when the dialog
    /// is closed.
    pub sort_draft: Option<crate::sort::SortChain>,
    /// `true` while the Sort Manager modal is visible.
    pub show_sort_manager: bool,
    /// True while the texture-validator game picker is visible.
    pub validator_popup_open: bool,
    /// Whether entry rows are tinted by their validator verdict.
    pub compat_highlight_enabled: bool,
    /// Archive-tab width in logical pixels (draggable divider).
    pub archive_tab_width: f32,
    /// Active tab-width drag; holds the last cursor x (NaN before the
    /// first move) so width changes track deltas without needing the
    /// strip's absolute origin.
    pub tab_resize_drag: Option<f32>,
    /// Whether `save_config()` may write to disk. Tests disable this so
    /// they never touch the user's real settings.ini.
    pub config_persist_enabled: bool,
    /// In-flight drag-and-drop between archive tabs. `None` when no
    /// drag is in progress. Holds the source archive + the entry
    /// indices being moved + the currently-hovered target. The
    /// `Drop` impl on this struct also implements "the drag was
    /// cancelled" — see the `clear` method — so dropping the
    /// value without committing cleanly resets the UI.
    pub drag_state: Option<crate::ui::drag::DragState>,
    pub last_export_selected_only: bool,
    pub panes: pane_grid::State<Pane>,
    pub context_menu: Option<(usize, usize)>,
    pub inspected_entry: Option<(usize, EntryInspection)>,
    /// Index into the decoded textures currently being viewed.
    pub selected_texture: usize,
    /// Whether the texture tab should draw the matching NIF UV layout.
    pub show_texture_uv: bool,
    /// Texture tab: Photoshop-style grid overlay.
    pub show_texture_grid: bool,
    /// Grid cells per axis for the texture grid overlay.
    pub texture_grid_divisions: u32,
    /// Archive/entry identity of the scene currently shown in the 3D viewer.
    /// Selection can change without destroying the current scene, so the UI
    /// uses this identity to avoid presenting a stale model as the new one.
    pub active_viewer_entry: Option<(usize, usize)>,
    /// Loading state for a cold 3D scene. Cache hits deliberately skip it.
    pub(crate) viewer_load: Option<ViewerLoadState>,
    /// Normalized position for the indeterminate model-loading spinner.
    pub(crate) viewer_load_phase: f32,
    pub scroll_y: f32,
    pub selected_inspector_tab: InspectorTab,
    pub viewer3d_handle: std::sync::Arc<crate::ui::viewer3d_widget::SceneHandle>,
    /// True when the search text has changed but the filtered list has not
    /// been updated yet. The filter is applied on a debounce tick so typing
    /// stays responsive even with large archives.
    pub filter_pending: bool,
    /// Mirrors the native Iced scrollable's active autoscroll mode so rows
    /// can stay inert until the next click stops it.
    pub autoscroll: bool,
    autoscroll_momentum: AutoScrollMomentum,
    /// Exact vertical range reported by the native Scrollable. The optional
    /// tail uses it to clamp its virtual offset to the real viewport range.
    entry_table_max_scroll_y: f32,
    entry_table_viewport_known: bool,
    entry_table_hovered: bool,
    /// Native-autoscroll control notice shown once per session.
    autoscroll_notice_shown: bool,
    pub modifiers: Modifiers,
    viewer_rxs: Vec<tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>>,
    pub animator: Animator,
    prev_tick: Option<std::time::Instant>,
    last_pointer_position: Option<Point>,
    entry_feedback_target: Option<(usize, usize)>,
    archive_tab_feedback_target: Option<usize>,
    inspector_tab_feedback_target: Option<InspectorTab>,
    ripple: Option<RippleState>,
    toast_pulses_remaining: u32,
    toast_pulse_target: f32,
    toast_start: Option<std::time::Instant>,
    /// How long the current toast stays up before auto-dismissal. Most
    /// toasts use the snappy default; the autoscroll notice uses
    /// a long duration so users can actually read the controls.
    toast_dismiss_after: Duration,
    /// Set by the autoscroll notice so the next toast-reveal
    /// tick adopts the long duration instead of the default.
    toast_extended_duration: bool,
    /// Text of the floating toast snackbar while it is visible or fading
    /// out. Mirrors `toast` but survives dismissal for the fade-out.
    pub(crate) toast_reveal_text: Option<String>,
    /// True while the toast snackbar is animating towards hidden.
    toast_reveal_fading: bool,
    /// Repeating 0..1 clock for the progress-bar shimmer sweep.
    pub(crate) shimmer_phase: f32,
    /// Repeating 0..1 clock for the empty-state idle animation.
    pub(crate) empty_state_phase: f32,
    /// Decoded 3D scenes keyed by (archive file name, archive generation,
    /// entry index). Lets the viewer restore a previously loaded model
    /// instantly instead of re-reading + re-parsing the NIF. Memory bound
    /// via a byte-budgeted `quick_cache` LRU; invalidation is driven by the
    /// archive's `generation` counter (see `ArchiveInfo::invalidate_entry_caches`).
    scene_cache: SceneCache,
    /// Memoized `IdeMap` per game root directory. Building walks the game
    /// folder on disk, so it's built at most once per archive path per
    /// session and shared across loads by `Arc`.
    ide_maps: HashMap<PathBuf, std::sync::Arc<crate::inspector::texture::IdeMap>>,
}

/// Cache key for [`App::scene_cache`]. Uses the archive's unique file name
/// (see `Editor::archive_exists_by_name`) rather than the archive index, so
/// closing an archive cannot re-key stale scenes onto a different archive.
/// The generation counter folds in entry-list mutations.
type SceneCacheKey = (String, u64, usize);
/// The scene cache doubles as the single-flight registry for scene loads:
/// `get_value_or_guard` hands out a placeholder guard to exactly one loader,
/// concurrent requesters see a zero-timeout `Timeout`, and the loader
/// publishes the scene atomically with `guard.insert`. `Arc`-wrapped so a
/// guard can be claimed inside an async load task.
type SceneCache = std::sync::Arc<
    quick_cache::sync::Cache<
        SceneCacheKey,
        Arc<crate::inspector::scene3d::Scene>,
        SceneCpuWeight,
    >,
>;

/// Weighs a cached scene by its estimated CPU memory (mesh buffers + decoded
/// RGBA textures), reusing the same estimate the GPU admission check uses.
#[derive(Clone)]
struct SceneCpuWeight;

impl quick_cache::Weighter<SceneCacheKey, Arc<crate::inspector::scene3d::Scene>>
    for SceneCpuWeight
{
    fn weight(&self, _key: &SceneCacheKey, val: &Arc<crate::inspector::scene3d::Scene>) -> u64 {
        val.estimated_gpu_bytes().unwrap_or(0).max(1)
    }
}

/// Soft memory budget for the scene cache. Keeps roughly the last few dozen
/// typical game models (vertices + textures) resident. The cache is lazily
/// filled, so this is a ceiling, not an upfront allocation. Mobile targets
/// get a smaller ceiling: per-app memory budgets there are tight and the
/// OS kills processes that grow too large (jetsam/LMK), so evicting and
/// re-decoding a scene in milliseconds is the better trade.
#[cfg(any(target_os = "android", target_os = "ios"))]
const SCENE_CACHE_WEIGHT_CAPACITY: u64 = 64 * 1024 * 1024;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const SCENE_CACHE_WEIGHT_CAPACITY: u64 = 256 * 1024 * 1024;
const SCENE_CACHE_ITEM_CAPACITY: usize = 256;

/// A memoized `IdeMap` paired with the game root it was built from, shipped
/// back to the app by a 3D load task for memoization.
type BuiltIdeMap = (PathBuf, Arc<crate::inspector::texture::IdeMap>);

impl Default for App {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl App {
    pub fn new(config: Config) -> Self {
        let show_welcome = !config.first_run_complete;
        let mut editor = Editor::new();
        editor.set_default_sort_chain(config.default_sort_chain.clone());
        editor.file_type_literal = config.literal_file_types;
        editor.context_selection_accumulates = config.context_selection_accumulates;
        // View preferences are mirrored onto App fields so the view
        // builder doesn't reach through `self.config` for hot UI state.
        let show_texture_grid = config.show_texture_grid;
        let texture_grid_divisions = config.texture_grid_divisions;
        let viewer3d_handle = std::sync::Arc::new(crate::ui::viewer3d_widget::SceneHandle::new());
        viewer3d_handle.set_navigation_visible(config.show_navigation_gizmo);
        let (panes, pane) = pane_grid::State::new(Pane::Table);
        let mut panes = panes;
        panes.split(pane_grid::Axis::Vertical, pane, Pane::Info);

        Self {
            editor,
            archive_tab_width: crate::config::clamp_archive_tab_width(config.archive_tab_width),
            config,
            sort_draft: None,
            show_sort_manager: false,
            validator_popup_open: false,
            compat_highlight_enabled: true,
            tab_resize_drag: None,
            config_persist_enabled: true,
            drag_state: None,
            last_export_selected_only: false,
            search: String::new(),
            rename_buffer: String::new(),
            search_focused: false,
            rename_focused: false,
            search_predictions: Vec::new(),
            prediction_index: None,
            did_you_mean: None,
            predictions_dismissed: false,
            pending_shortcut: None,
            pending_search_focus: None,
            pending_rename_focus: None,
            show_about: false,
            show_welcome,
            welcome_persist: true,
            show_unsupported: None,
            show_update_status: None,
            update_state: UpdateState::Idle,
            update_check_manual: false,
            toast: None,
            pending_folder_import: None,
            pending_import: None,
            pending_save: None,
            replace_request: None,
            replace_attempt: 0,
            new_txd_attempt: 0,
            new_txd_picker_open: false,
            new_txd_plan_in_flight: false,
            replace_plan_in_flight: false,
            pending_replace: None,
            pending_new_txd: None,
            pending_bulk: None,
            pending_close: None,
            close_after_save: None,
            quitting: None,
            panes,
            context_menu: None,
            inspected_entry: None,
            selected_texture: 0,
            show_texture_uv: false,
            show_texture_grid,
            texture_grid_divisions,
            active_viewer_entry: None,
            viewer_load: None,
            viewer_load_phase: 0.0,
            scroll_y: 0.0,
            filter_pending: false,
            autoscroll: false,
            autoscroll_momentum: AutoScrollMomentum::default(),
            entry_table_max_scroll_y: 0.0,
            entry_table_viewport_known: false,
            entry_table_hovered: false,
            autoscroll_notice_shown: false,
            modifiers: Modifiers::default(),
            viewer_rxs: Vec::new(),
            animator: Animator::new(),
            prev_tick: None,
            last_pointer_position: None,
            entry_feedback_target: None,
            archive_tab_feedback_target: None,
            inspector_tab_feedback_target: None,
            ripple: None,
            toast_pulses_remaining: 0,
            toast_pulse_target: 0.0,
            toast_start: None,
            toast_dismiss_after: Duration::from_millis(2500),
            toast_extended_duration: false,
            toast_reveal_text: None,
            toast_reveal_fading: false,
            shimmer_phase: 0.0,
            empty_state_phase: 0.0,
            selected_inspector_tab: InspectorTab::Export,
            viewer3d_handle,
            scene_cache: std::sync::Arc::new(quick_cache::sync::Cache::with(
                SCENE_CACHE_ITEM_CAPACITY,
                SCENE_CACHE_WEIGHT_CAPACITY,
                SceneCpuWeight,
                Default::default(),
                Default::default(),
            )),
            ide_maps: HashMap::new(),
        }
    }

    pub fn theme(&self) -> Theme {
        resolve_theme(self.config.theme)
    }

    /// The design-token system for the current theme.
    pub fn design(&self) -> Design {
        let tokens = if matches!(self.config.theme, ThemeMode::DarkEverforest) {
            crate::ui::tokens::ThemeTokens::everforest()
        } else if self.theme().extended_palette().is_dark {
            crate::ui::tokens::ThemeTokens::dark()
        } else {
            crate::ui::tokens::ThemeTokens::light()
        };
        Design::from_tokens(tokens, self.theme().extended_palette().is_dark)
    }

    pub fn startup_task(config: &Config) -> Task<Message> {
        let mut tasks = vec![iced::font::load(LUCIDE_FONT_BYTES).map(|_| Message::Noop)];
        if config.update_check_enabled {
            tasks.push(Task::perform(
                check_updates_future(
                    UPDATER_REPO.to_string(),
                    env!("CARGO_PKG_VERSION").to_string(),
                ),
                Message::UpdateResultReceived,
            ));
        }
        Task::batch(tasks)
    }

    pub fn save_config(&self) {
        if !self.config_persist_enabled {
            return;
        }
        if let Err(err) = self.config.save() {
            eprintln!("failed to save config: {err}");
        }
    }

    /// Re-apply the persisted target to an archive (idempotent; safe to call
    /// after tasks that rebuild the archive). Takes `&Config` so callers
    /// can hold a mutable borrow of the archive's field.
    fn adopt_target(config: &crate::config::Config, archive: &mut ArchiveInfo) {
        let Some(path) = archive.path.clone() else {
            return;
        };
        archive.target_game = config
            .archive_target(&path)
            .and_then(crate::compat::games::profile_by_id)
            .map(|game| game.id);
    }

    /// Give freshly imported entries their row verdicts. Imports keep the
    /// existing compatibility report (indices are stable), so only the
    /// imported entries are checked and merged in - the rest of the
    /// archive keeps its previous tinting.
    fn refresh_imported_verdicts(&mut self, archive_index: usize) {
        let target = self
            .editor
            .archives()
            .get(archive_index)
            .and_then(|archive| archive.target_game)
            .and_then(crate::compat::games::profile_by_id);
        let Some(target) = target else {
            return;
        };
        let Some(archive) = self.editor.archives_mut().get_mut(archive_index) else {
            return;
        };
        if archive.compat_report.is_none() {
            return;
        }
        let imported: Vec<(usize, String, PathBuf)> = archive
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.imported)
            .filter_map(|(index, entry)| {
                Some((index, entry.file_name.to_string(), entry.source_path.clone()?))
            })
            .collect();
        let verdicts: Vec<crate::compat::scan::EntryVerdict> = imported
            .into_iter()
            .filter_map(|(entry_index, file_name, path)| {
                let check = crate::compat::scan::check_import_file(&path, target);
                if check.textures == 0 {
                    return None;
                }
                Some(crate::compat::scan::EntryVerdict {
                    entry_index,
                    file_name,
                    textures: check.textures,
                    worst: check.worst,
                    counts: check.counts,
                })
            })
            .collect();
        if verdicts.is_empty() {
            return;
        }
        if let Some(report) = archive.compat_report.as_mut() {
            report
                .entry_verdicts
                .retain(|existing| !verdicts.iter().any(|new| new.entry_index == existing.entry_index));
            report.entry_verdicts.extend(verdicts);
        }
    }

    fn open_archive_path(&mut self, path: PathBuf) -> Task<Message> {
        let result_path = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if crate::parser::detect_version(&path) == ImgVersion::Unknown {
                        return OpenArchiveOutcome::Unsupported;
                    }

                    match ArchiveInfo::open(path) {
                        Ok(archive) => OpenArchiveOutcome::Opened(Box::new(archive)),
                        Err(error) => OpenArchiveOutcome::Failed(error.to_string()),
                    }
                })
                .await
                .unwrap_or_else(|error| {
                    OpenArchiveOutcome::Failed(format!("open task panicked: {error}"))
                })
            },
            move |outcome| Message::ArchiveOpenCompleted {
                path: result_path,
                outcome,
            },
        )
    }

    /// Start an import. When the archive has a validator target, run the
    /// pre-flight format check first; only flagged files open the dialog.
    fn begin_import(
        &mut self,
        index: usize,
        archive: ArchiveInfo,
        paths: Vec<PathBuf>,
        folder: Option<(FolderImportPlan, FolderDuplicatePolicy)>,
    ) -> Task<Message> {
        let Some(target) = archive
            .target_game
            .and_then(crate::compat::games::profile_by_id)
        else {
            return match folder {
                Some((plan, policy)) => self.run_folder_import(index, archive, plan, policy),
                None => Self::import_archive_task(index, archive, paths, false),
            };
        };
        let check_paths = paths.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    check_paths
                        .iter()
                        .map(|path| crate::compat::scan::check_import_file(path, target))
                        .collect::<Vec<_>>()
                })
                .await
                .map_err(|error| error.to_string())
            },
            move |result| Message::ImportPreflightCompleted {
                index,
                paths,
                folder: folder.map(Box::new),
                checks: result.unwrap_or_default(),
            },
        )
    }

    fn import_archive_task(
        index: usize,
        archive: ArchiveInfo,
        paths: Vec<PathBuf>,
        checked: bool,
    ) -> Task<Message> {
        let count = paths.len();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut archive = archive;
                    Editor::append_import_to(&mut archive, &paths, false);
                    archive
                })
                .await
                .map_err(|error| format!("import task panicked: {error}"))
            },
            move |result| Message::ImportCompleted {
                index,
                count,
                checked,
                result,
            },
        )
    }

    pub fn visit_repository() {
        let _ = webbrowser::open(REPO_URL);
    }

    pub fn has_active_progress(&self) -> bool {
        self.editor.has_active_progress()
    }

    fn refresh_inspection(&mut self) -> Task<Message> {
        let selected_archive = self.editor.selected_archive();
        let selected_entry = self.editor.selected_entry();

        let (Some(archive_index), Some(entry_index)) = (selected_archive, selected_entry) else {
            self.inspected_entry = None;
            return Task::none();
        };

        // Inspections are served synchronously: the parse reads only an
        // 8 KiB header slice from the mmap, and the per-archive cache
        // keeps repeat selections instant. A previous async fallback here
        // was unreachable (the cached lookup always resolves for a valid
        // entry index) and has been removed.
        self.inspected_entry = {
            let archive = self.editor.archives_mut().get_mut(archive_index);
            archive
                .and_then(|archive| inspect_entry_cached(archive, entry_index))
                .map(|inspection| (entry_index, inspection))
        };
        Task::none()
    }

    fn display_row_to_entry(&self, display_row: usize) -> Option<usize> {
        self.editor
            .selected_archive()
            .and_then(|_| {
                self.editor
                    .archives()
                    .get(self.editor.selected_archive().unwrap_or(0))
            })
            .and_then(|a| a.selected_indices.get(display_row).copied())
    }

    fn run_refresh_filter(&mut self) -> Task<Message> {
        self.editor.update_filtered_list(&self.search);
        self.refresh_search_predictions();
        Task::none()
    }

    /// True when the prediction dropdown should be rendered: the search
    /// input has focus, the query produced suggestions, and the user
    /// has not dismissed them with Escape.
    pub(crate) fn predictions_open(&self) -> bool {
        self.search_focused
            && !self.predictions_dismissed
            && (!self.search_predictions.is_empty() || self.did_you_mean.is_some())
    }

    /// Recompute the prediction dropdown from the current query. Cheap
    /// enough to run on every debounced filter refresh (one scored pass
    /// over the selected archive's entry names).
    fn refresh_search_predictions(&mut self) {
        self.prediction_index = None;
        self.did_you_mean = None;
        self.search_predictions.clear();
        let query = self.search.trim().to_lowercase();
        if query.is_empty() || !self.config.show_search_bar {
            return;
        }
        let Some(archive_index) = self.editor.selected_archive() else {
            return;
        };
        let Some(archive) = self.editor.archives().get(archive_index) else {
            return;
        };

        let mut scored: Vec<(i64, usize)> = archive
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                crate::search::fuzzy_score(&entry.file_name_lower, &query)
                    .map(|score| (score, index))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| {
                archive.entries[a.1]
                    .file_name
                    .cmp(&archive.entries[b.1].file_name)
            })
        });
        scored.truncate(MAX_SEARCH_PREDICTIONS);
        self.search_predictions = scored
            .iter()
            .map(|&(_, index)| (index, archive.entries[index].file_name.to_string()))
            .collect();

        if self.search_predictions.is_empty() {
            let mut best: Option<(f64, usize)> = None;
            for (index, entry) in archive.entries.iter().enumerate() {
                let similarity =
                    fuzzt::algorithms::jaro_winkler(&entry.file_name_lower, &query);
                if best.is_none_or(|(current, _)| similarity > current) {
                    best = Some((similarity, index));
                }
            }
            if let Some((similarity, index)) = best
                && similarity >= crate::search::DID_YOU_MEAN_MIN_SIMILARITY
            {
                self.did_you_mean = Some((index, archive.entries[index].file_name.to_string()));
            }
        }
    }

    /// Commit a prediction: adopt its full name as the query, filter to
    /// it, select it like a row click, and scroll the table to the top
    /// (the exact match always sorts first).
    fn commit_search_prediction(&mut self, entry_index: usize) -> Task<Message> {
        self.close_predictions();
        let Some(archive_index) = self.editor.selected_archive() else {
            return Task::none();
        };
        let name = self
            .editor
            .archives()[archive_index]
            .entries
            .get(entry_index)
            .map(|entry| entry.file_name.to_string());
        let Some(name) = name else {
            return Task::none();
        };
        self.search = name;
        self.editor.update_filtered_list(&self.search);
        // The exact match sorts first; keep the virtual offset in sync
        // with the scroll-to-top.
        self.scroll_y = 0.0;
        let click_task = self.update(Message::EntryClicked(0));
        Task::batch(vec![
            click_task,
            // Refocusing the text input also moves its caret to the end
            // of the committed name (State::focus resets the cursor), so
            // the insertion point doesn't linger at the old typed offset.
            iced::widget::operation::focus(iced::widget::Id::new(SEARCH_INPUT_ID)),
            iced::advanced::widget::operate(scroll_to(
                iced::widget::Id::new("entry_table"),
                AbsoluteOffset {
                    x: None,
                    y: Some(0.0),
                },
            )),
        ])
    }
    fn close_predictions(&mut self) {
        self.search_predictions.clear();
        self.did_you_mean = None;
        self.prediction_index = None;
        self.predictions_dismissed = true;
    }

    /// Like `iced::widget::operation::is_focused`, but always completes:
    /// the stock operation finishes with `Outcome::None` when the target
    /// widget is absent from the tree (the search box on the welcome
    /// screen, the rename box outside rename mode), which silently drops
    /// the mapped message and deadlocks the shortcut focus handshake.
    /// A missing widget here simply counts as "not focused".
    fn is_focused_or_absent(id: iced::widget::Id) -> Task<bool> {
        use iced::Rectangle;
        use iced::advanced::widget::operation::{Focusable, Operation, Outcome};

        struct Probe {
            target: iced::widget::Id,
            result: Option<bool>,
        }

        impl Operation<bool> for Probe {
            fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<bool>)) {
                operate(self);
            }

            fn focusable(
                &mut self,
                id: Option<&iced::widget::Id>,
                _bounds: Rectangle,
                state: &mut dyn Focusable,
            ) {
                if id.is_some_and(|id| *id == self.target) {
                    self.result = Some(state.is_focused());
                }
            }

            fn finish(&self) -> Outcome<bool> {
                Outcome::Some(self.result.unwrap_or(false))
            }
        }

        iced::advanced::widget::operate(Probe {
            target: id,
            result: None,
        })
    }

    fn input_focus_task() -> Task<Message> {
        Task::batch(vec![
            Self::is_focused_or_absent(iced::widget::Id::new(SEARCH_INPUT_ID))
                .map(Message::SearchFocusChanged),
            Self::is_focused_or_absent(iced::widget::Id::new(RENAME_INPUT_ID))
                .map(Message::RenameFocusChanged),
        ])
    }

    fn begin_shortcut_focus_check(&mut self, shortcut: Shortcut) -> Task<Message> {
        self.pending_shortcut = Some(shortcut);
        self.pending_search_focus = None;
        self.pending_rename_focus = None;
        Self::input_focus_task()
    }

    /// True while a modal dialog covers the workspace. Keyboard shortcuts
    /// must not fire behind a dialog (a stray `1`/`2`/`3` or Ctrl+D while
    /// reading the About box would otherwise act on the hidden UI).
    pub(crate) fn modal_open(&self) -> bool {
        self.show_about
            || self.show_welcome
            || self.show_unsupported.is_some()
            || self.pending_folder_import.is_some()
            || self.pending_import.is_some()
            || self.pending_save.is_some()
            || self.pending_replace.is_some()
            || self.pending_new_txd.is_some()
            || self.pending_bulk.is_some()
            || self.pending_close.is_some()
            || self.show_update_status.is_some()
            || self.show_sort_manager
            || self.validator_popup_open
    }

    /// Open the validator popup for the selected archive, probing its
    /// content for a target suggestion the first time. Shared by the
    /// toolbar button and by converter actions that need the user to
    /// pick (or fix) the archive's game target.
    fn open_validator_popup(&mut self) -> Task<Message> {
        let Some(archive_index) = self.editor.selected_archive() else {
            self.toast = Some("Open an archive first to validate it.".into());
            return Task::none();
        };
        if self.editor.archives()[archive_index].progress.in_use() {
            self.toast = Some("Another task is still running.".into());
            return Task::none();
        }
        self.validator_popup_open = true;
        // Probe the content once so the picker can suggest a game.
        if self.editor.archives()[archive_index].target_hint.is_none() {
            let snapshot = self.editor.archives()[archive_index].clone();
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::compat::hint::probe_target(
                            &snapshot,
                            crate::compat::hint::PROBE_SAMPLE_LIMIT,
                        )
                    })
                    .await
                    .unwrap_or(None)
                },
                move |hint| Message::TargetProbed {
                    archive_index,
                    hint,
                },
            );
        }
        Task::none()
    }

    fn resolve_shortcut_focus_check(&mut self) -> Task<Message> {
        let (Some(search_focused), Some(rename_focused)) =
            (self.pending_search_focus, self.pending_rename_focus)
        else {
            return Task::none();
        };
        let Some(shortcut) = self.pending_shortcut.take() else {
            return Task::none();
        };

        self.pending_search_focus = None;
        self.pending_rename_focus = None;
        self.search_focused = search_focused;
        self.rename_focused = rename_focused;

        if search_focused || rename_focused || self.modal_open() {
            Task::none()
        } else {
            self.handle_shortcut(shortcut)
        }
    }

    pub(crate) fn selected_entry_key(&self) -> Option<(usize, usize)> {
        Some((
            self.editor.selected_archive()?,
            self.editor.selected_entry()?,
        ))
    }

    fn interaction_duration(&self, preset: DurationPreset) -> Duration {
        let motion = self.design().tokens.motion.get(preset);
        Duration::from_millis(u64::from(motion.duration_ms))
    }

    fn prepare_interaction_animation(&mut self) {
        // The animation subscription is intentionally stopped while idle. Do
        // not let the first tick of a new effect inherit the elapsed wall time
        // from the previous subscription. Continuous idle effects (toast
        // reveal, progress shimmer, empty-state breathing) keep the
        // subscription alive, so the tick chain must not be reset while any
        // of them could be driving frames — otherwise their phase clocks
        // never advance.
        let subscription_alive = self.animator.running_count() > 0
            || self.toast.is_some()
            || self.toast_reveal_text.is_some()
            || self.viewer_load.is_some()
            || self.has_active_progress()
            || self.autoscroll_momentum.is_active()
            || (self.editor.archives().is_empty() && self.config.motion_enabled);
        if !subscription_alive {
            self.prev_tick = None;
        }
    }

    fn end_autoscroll(&mut self) {
        self.autoscroll = false;
        self.autoscroll_momentum.clear();
    }

    fn update_autoscroll_momentum(&mut self, position: Point) {
        if !self.autoscroll {
            return;
        }

        if self
            .autoscroll_momentum
            .update_pointer(position, self.config.autoscroll_momentum_enabled)
        {
            self.prepare_interaction_animation();
        }
    }

    fn advance_autoscroll_momentum(&mut self, dt: Duration) -> Task<Message> {
        if !self.autoscroll || !self.config.autoscroll_momentum_enabled {
            self.autoscroll_momentum.clear_tail();
            return Task::none();
        }
        if !self.entry_table_viewport_known {
            return Task::none();
        }

        let max_y = self.entry_table_max_scroll_y;
        if !max_y.is_finite()
            || max_y <= 0.0
            || !self.scroll_y.is_finite()
            || self.scroll_y < 0.0
            || self.scroll_y > max_y
        {
            self.autoscroll_momentum.clear_tail();
            return Task::none();
        }

        let Some(delta) = self.autoscroll_momentum.advance(dt) else {
            return Task::none();
        };

        let target_y = (self.scroll_y + delta).clamp(0.0, max_y);
        if (target_y - self.scroll_y).abs() <= f32::EPSILON {
            self.autoscroll_momentum.clear_tail();
            return Task::none();
        }

        self.scroll_y = target_y;
        iced::advanced::widget::operate(scroll_to(
            iced::widget::Id::new("entry_table"),
            AbsoluteOffset {
                x: None,
                y: Some(target_y),
            },
        ))
    }

    fn start_entry_feedback(&mut self, target: (usize, usize)) {
        if self.config.motion_enabled && self.config.selection_pulse_enabled {
            self.prepare_interaction_animation();
            self.entry_feedback_target = Some(target);
            self.animator.animate(
                ANIM_ENTRY_FEEDBACK,
                0.0,
                1.0,
                self.interaction_duration(DurationPreset::Normal),
                crate::ui::easing::Easing::CubicOut,
            );
        } else {
            self.entry_feedback_target = None;
            self.animator.cancel(ANIM_ENTRY_FEEDBACK);
        }
    }

    fn start_archive_tab_feedback(&mut self, target: usize) {
        if self.config.motion_enabled && self.config.selection_pulse_enabled {
            self.prepare_interaction_animation();
            self.archive_tab_feedback_target = Some(target);
            self.animator.animate(
                ANIM_ARCHIVE_TAB_FEEDBACK,
                0.0,
                1.0,
                self.interaction_duration(DurationPreset::Normal),
                crate::ui::easing::Easing::CubicOut,
            );
        } else {
            self.archive_tab_feedback_target = None;
            self.animator.cancel(ANIM_ARCHIVE_TAB_FEEDBACK);
        }
    }

    fn start_inspector_tab_feedback(&mut self, target: InspectorTab) {
        if self.config.motion_enabled && self.config.selection_pulse_enabled {
            self.prepare_interaction_animation();
            self.inspector_tab_feedback_target = Some(target);
            self.animator.animate(
                ANIM_INSPECTOR_TAB_FEEDBACK,
                0.0,
                1.0,
                self.interaction_duration(DurationPreset::Normal),
                crate::ui::easing::Easing::CubicOut,
            );
        } else {
            self.inspector_tab_feedback_target = None;
            self.animator.cancel(ANIM_INSPECTOR_TAB_FEEDBACK);
        }
    }

    fn start_click_ripple(&mut self, target: RippleTarget) {
        if self.config.motion_enabled && self.config.click_ripple_enabled {
            self.prepare_interaction_animation();
            self.ripple = Some(RippleState {
                target,
                origin: self.last_pointer_position,
            });
            self.animator.animate(
                ANIM_CLICK_RIPPLE,
                0.0,
                1.0,
                self.interaction_duration(DurationPreset::Slow),
                crate::ui::easing::Easing::CubicOut,
            );
        } else {
            self.ripple = None;
            self.animator.cancel(ANIM_CLICK_RIPPLE);
        }
    }

    fn stop_interaction_animations(&mut self) {
        self.animator.cancel(ANIM_ENTRY_FEEDBACK);
        self.animator.cancel(ANIM_ARCHIVE_TAB_FEEDBACK);
        self.animator.cancel(ANIM_INSPECTOR_TAB_FEEDBACK);
        self.animator.cancel(ANIM_CLICK_RIPPLE);
        self.entry_feedback_target = None;
        self.archive_tab_feedback_target = None;
        self.inspector_tab_feedback_target = None;
        self.ripple = None;
    }

    fn pulse_value(&self, id: crate::ui::animator::AnimationId) -> f32 {
        if !self.animator.is_running(id) {
            return 0.0;
        }
        (self.animator.get(id) * std::f32::consts::PI)
            .sin()
            .max(0.0)
    }

    pub(crate) fn entry_selection_pulse(&self, target: (usize, usize)) -> f32 {
        if !self.config.motion_enabled
            || !self.config.selection_pulse_enabled
            || self.entry_feedback_target != Some(target)
        {
            return 0.0;
        }
        self.pulse_value(ANIM_ENTRY_FEEDBACK)
    }

    pub(crate) fn entry_icon_nudge(&self, target: (usize, usize)) -> f32 {
        if !self.config.motion_enabled
            || !self.config.icon_micro_motion_enabled
            || self.entry_feedback_target != Some(target)
            || !self.animator.is_running(ANIM_ENTRY_FEEDBACK)
        {
            return 0.0;
        }
        let progress = self.animator.get(ANIM_ENTRY_FEEDBACK);
        (progress * std::f32::consts::PI * 3.0).sin() * 2.0
    }

    pub(crate) fn entry_text_nudge(&self, target: (usize, usize)) -> f32 {
        if !self.config.motion_enabled
            || !self.config.selection_pulse_enabled
            || self.entry_feedback_target != Some(target)
            || !self.animator.is_running(ANIM_ENTRY_FEEDBACK)
        {
            return 0.0;
        }
        let progress = self.animator.get(ANIM_ENTRY_FEEDBACK);
        (progress * std::f32::consts::PI * 4.0).sin() * 1.25
    }

    pub(crate) fn archive_tab_selection_pulse(&self, target: usize) -> f32 {
        if !self.config.motion_enabled
            || !self.config.selection_pulse_enabled
            || self.archive_tab_feedback_target != Some(target)
        {
            return 0.0;
        }
        self.pulse_value(ANIM_ARCHIVE_TAB_FEEDBACK)
    }

    pub(crate) fn inspector_tab_selection_pulse(&self, target: InspectorTab) -> f32 {
        if !self.config.motion_enabled
            || !self.config.selection_pulse_enabled
            || self.inspector_tab_feedback_target != Some(target)
        {
            return 0.0;
        }
        self.pulse_value(ANIM_INSPECTOR_TAB_FEEDBACK)
    }

    pub(crate) fn ripple_visual(
        &self,
        target: RippleTarget,
    ) -> Option<crate::ui::interaction::RippleVisual> {
        if !self.config.motion_enabled
            || !self.config.click_ripple_enabled
            || !self.animator.is_running(ANIM_CLICK_RIPPLE)
            || self.ripple.is_none_or(|ripple| ripple.target != target)
        {
            return None;
        }
        let ripple = self.ripple?;
        Some(crate::ui::interaction::RippleVisual {
            origin: ripple.origin,
            progress: self.animator.get(ANIM_CLICK_RIPPLE),
        })
    }

    pub(crate) fn viewer_scene_matches_selection(&self) -> bool {
        self.active_viewer_entry == self.selected_entry_key()
            && self.viewer3d_handle.with(|inner| inner.scene.is_some())
    }

    pub(crate) fn viewer_load_matches_selection(&self) -> bool {
        self.viewer_load
            .as_ref()
            .is_some_and(|load| Some(load.target) == self.selected_entry_key())
    }

    pub(crate) fn viewer_loading_entry_name(&self) -> Option<&str> {
        self.viewer_load
            .as_ref()
            .filter(|load| Some(load.target) == self.selected_entry_key())
            .map(|load| load.entry_name.as_str())
    }

    /// The floating toast snackbar's text and reveal progress (0 = hidden,
    /// 1 = fully shown). Returns `Some` while the toast is up and during
    /// its fade-out, so the view layer gets a continuous lifecycle.
    pub(crate) fn toast_overlay(&self) -> Option<(String, f32)> {
        let text = self.toast_reveal_text.clone()?;
        let reveal = if !self.config.motion_enabled {
            1.0
        } else if self.toast_reveal_fading {
            self.animator.get_or(ANIM_TOAST_REVEAL, 0.0).clamp(0.0, 1.0)
        } else {
            self.animator.get_or(ANIM_TOAST_REVEAL, 1.0).clamp(0.0, 1.0)
        };
        (reveal > 0.0).then_some((text, reveal))
    }

    fn begin_viewer_load(&mut self, target: (usize, usize), entry_name: String) {
        if self
            .viewer_load
            .as_ref()
            .is_some_and(|load| load.target == target)
        {
            return;
        }
        // The animation clock is intentionally idle when no other effect is
        // running. Reset it here so a first loader frame never skips ahead.
        if self.animator.running_count() == 0 && self.toast.is_none() {
            self.prev_tick = None;
        }
        self.viewer_load = Some(ViewerLoadState { target, entry_name });
        self.viewer_load_phase = 0.0;
    }

    fn clear_viewer_load(&mut self) {
        self.viewer_load = None;
        self.viewer_load_phase = 0.0;
    }

    fn clear_stale_viewer_load(&mut self) {
        if self.viewer_load.is_some() && !self.viewer_load_matches_selection() {
            self.clear_viewer_load();
        }
    }

    fn reset_texture_preview_state(&mut self) {
        self.selected_texture = 0;
        self.show_texture_uv = false;
    }

    /// Keep the active inspector tab in sync with the selected previewable
    /// entry. This is intentionally limited to the tab the user is already
    /// viewing, so ordinary archive browsing does not unexpectedly steal
    /// focus from the export/info panel.
    fn refresh_active_preview(&mut self) -> Task<Message> {
        match self.selected_inspector_tab {
            InspectorTab::Model3D => {
                let is_model = self
                    .editor
                    .selected_archive()
                    .and_then(|archive_index| self.editor.archives().get(archive_index))
                    .and_then(|archive| {
                        self.editor
                            .selected_entry()
                            .and_then(|entry_index| archive.entries.get(entry_index))
                    })
                    .is_some_and(|entry| is_renderable_model_name(&entry.file_name));
                if is_model {
                    self.load_selected_nif(InspectorTab::Model3D)
                } else {
                    self.clear_viewer_load();
                    Task::none()
                }
            }
            InspectorTab::Texture => self.load_selected_texture(),
            InspectorTab::Export => Task::none(),
        }
    }

    fn load_selected_texture(&mut self) -> Task<Message> {
        let Some(archive_index) = self.editor.selected_archive() else {
            return Task::none();
        };
        let Some(entry_index) = self.editor.selected_entry() else {
            return Task::none();
        };
        let Some(entry) = self
            .editor
            .archives()
            .get(archive_index)
            .and_then(|archive| archive.entries.get(entry_index))
        else {
            return Task::none();
        };

        let lower = entry.file_name.to_ascii_lowercase();
        self.selected_inspector_tab = InspectorTab::Texture;
        self.reset_texture_preview_state();
        if lower.ends_with(".nif") || lower.ends_with(".dff") {
            // Model textures are resolved through the scene decoder so the
            // texture tab and UV overlay share the same source of truth.
            if self.viewer_scene_matches_selection() {
                return Task::none();
            }
            return self.load_selected_nif(InspectorTab::Texture);
        }
        if !lower.ends_with(".txd") && !lower.ends_with(".nft") {
            return Task::none();
        }
        let cached = self
            .editor
            .archives()
            .get(archive_index)
            .is_some_and(|archive| archive.texture_cache.contains_key(&entry_index));
        if cached {
            Task::none()
        } else {
            self.decode_texture_entry(entry_index)
        }
    }

    fn load_selected_nif(&mut self, target_tab: InspectorTab) -> Task<Message> {
        let Some(archive_index) = self.editor.selected_archive() else {
            self.toast = Some("Select a NIF or DFF entry first.".into());
            return Task::none();
        };
        let Some(entry_index) = self.editor.selected_entry() else {
            self.toast = Some("Select a NIF or DFF entry first.".into());
            return Task::none();
        };
        let Some(entry) = self
            .editor
            .archives()
            .get(archive_index)
            .and_then(|archive| archive.entries.get(entry_index))
        else {
            self.toast = Some("The selected entry is no longer available.".into());
            return Task::none();
        };
        if !is_renderable_model_name(&entry.file_name) {
            self.toast = Some(format!(
                "In-app 3D viewer supports .nif and .dff ({}).",
                entry.file_name
            ));
            return Task::none();
        }

        self.selected_inspector_tab = target_tab;
        let target = (archive_index, entry_index);
        let entry_name = entry.file_name.to_string();
        if self.viewer_scene_matches_selection() {
            self.clear_viewer_load();
            return Task::none();
        }
        // A user can press the explicit load control while the automatic
        // selection load is in flight. Keep one decode task and one stable
        // transition instead of restarting the spinner or doing duplicate I/O.
        if self
            .viewer_load
            .as_ref()
            .is_some_and(|load| load.target == target)
        {
            return Task::none();
        }
        // Cache hit: the scene for this (archive, generation, entry) is
        // already decoded — restore it instantly instead of re-reading,
        // re-parsing, and re-resolving textures.
        if let Some(scene) = self.cached_scene_for(archive_index, entry_index) {
            self.clear_viewer_load();
            self.store_scene_texture_previews(&scene, archive_index, entry_index);
            self.viewer3d_handle.set_scene(scene);
            self.active_viewer_entry = Some((archive_index, entry_index));
            dev_logger::breadcrumb(&format!(
                "3D cache hit: entries {} (hits {}, misses {}, resident {:.1} MiB)",
                self.scene_cache.len(),
                self.scene_cache.hits(),
                self.scene_cache.misses(),
                self.scene_cache.weight() as f64 / (1024.0 * 1024.0),
            ));
            if let Some(archive) = self.editor.selected_archive_mut() {
                archive.add_log("In-app 3D viewer ready (cached)".to_string());
            }
            return Task::none();
        }
        self.active_viewer_entry = None;
        self.viewer3d_handle.clear();
        self.begin_viewer_load(target, entry_name);
        Task::done(Message::Viewer3dRequestLoad {
            archive_index,
            entry_index,
        })
    }

    /// Drop the scene cache entries belonging to one archive. The scene
    /// cache lives on `App` (not on `ArchiveInfo`), so closing an archive
    /// would otherwise leave its decoded scenes resident until budget
    /// pressure evicts them.
    fn drop_scene_cache_for_archive(&self, archive_name: &str) {
        self.scene_cache.retain(|key, _| key.0 != archive_name);
    }

    /// `Cache::retain` skips in-flight placeholders, so closing the archive
    /// that owns a load would otherwise let the late decode publish into
    /// the closed archive's key. Removing the key drops the placeholder;
    /// the worker's `guard.insert` then fails and discards the stale scene.
    fn drop_in_flight_placeholder(
        &self,
        archive_name: &str,
        generation: u64,
        in_flight: Option<(usize, usize)>,
        closed_index: usize,
    ) {
        if let Some(target) = in_flight.filter(|target| target.0 == closed_index) {
            self.scene_cache
                .remove(&(archive_name.to_string(), generation, target.1));
        }
    }

    /// Look up a decoded scene for the selected entry, keyed by the
    /// archive's stable file name + generation so stale entries miss.
    fn cached_scene_for(
        &mut self,
        archive_index: usize,
        entry_index: usize,
    ) -> Option<Arc<crate::inspector::scene3d::Scene>> {
        let key = {
            let archive = self.editor.archives().get(archive_index)?;
            (archive.file_name.clone(), archive.generation(), entry_index)
        };
        self.scene_cache.get(&key)
    }

    /// Derive texture previews from a decoded scene and store them in the
    /// archive's texture cache for the texture tab. Shared by the async
    /// load completion and the synchronous cache-hit restore.
    fn store_scene_texture_previews(
        &mut self,
        scene: &crate::inspector::scene3d::Scene,
        archive_index: usize,
        entry_index: usize,
    ) {
        let texture_previews = crate::ui::texture_preview::decoded_textures_from_scene(scene);
        if !texture_previews.is_empty()
            && let Some(archive) = self.editor.archives_mut().get_mut(archive_index)
        {
            archive
                .texture_cache
                .insert(entry_index, Arc::new(texture_previews));
        }
    }

    /// Close an archive tab, guarded: a dirty archive opens the
    /// unsaved-changes dialog instead of dropping the edits silently.
    fn request_archive_close(&mut self, index: usize) -> Task<Message> {
        let Some(archive) = self.editor.archives().get(index) else {
            return Task::none();
        };
        if archive.dirty {
            self.pending_close = Some(PendingClose::Archive(index));
            return Task::none();
        }
        self.close_archive_at(index)
    }

    /// Actually close the archive and clean up everything keyed to it.
    fn close_archive_at(&mut self, index: usize) -> Task<Message> {
        let closing = self
            .editor
            .archives()
            .get(index)
            .map(|archive| (archive.file_name.clone(), archive.generation()));
        let in_flight = self.viewer_load.as_ref().map(|load| load.target);
        self.editor.close_archive(index);
        self.active_viewer_entry = None;
        self.clear_viewer_load();
        self.viewer3d_handle.clear();
        if let Some((name, generation)) = closing {
            self.drop_scene_cache_for_archive(&name);
            self.drop_in_flight_placeholder(&name, generation, in_flight, index);
        }
        let task = self.refresh_inspection();
        Task::batch(vec![task, Task::none()])
    }

    /// Route a save through the pre-save check: silent when the stored
    /// report is clean (or no target is set), a review dialog when the
    /// target or consistency has issues, and a validation scan first
    /// when no report exists yet.
    fn begin_save(
        &mut self,
        archive: ArchiveInfo,
        path: PathBuf,
        version: crate::parser::ImgVersion,
        remove_existing: bool,
    ) -> Task<Message> {
        let index = self.editor.selected_archive().unwrap_or(0);
        if archive.target_game.is_none() {
            return self.run_save(archive, path, version, remove_existing);
        }

        let Some(report) = archive.compat_report.as_ref() else {
            // No report yet: validate first, then resume the save.
            self.pending_save = Some(PendingSave {
                index,
                path,
                version,
                remove_existing,
                issue: None,
                fix: false,
            });
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::compat::scan::validate_open_archive(
                            &archive,
                            &crate::compat::scan::ScanOptions {
                                decode_pixels: false,
                                target: archive.target_game,
                            },
                        )
                    })
                    .await
                    .unwrap_or_else(|err| Err(anyhow::anyhow!("task panicked: {err}")))
                },
                move |result| Message::CompatibilityValidated {
                    archive_index: index,
                    result: result.map_err(|err| format!("{err}")),
                },
            );
        };

        let issue = crate::compat::save::evaluate_save(report, &archive);
        if issue.needs_review() {
            let fix = issue.has_fixable();
            self.pending_save = Some(PendingSave {
                index,
                path,
                version,
                remove_existing,
                issue: Some(issue),
                fix,
            });
            Task::none()
        } else {
            self.run_save(archive, path, version, remove_existing)
        }
    }

    fn run_save(
        &self,
        archive: ArchiveInfo,
        path: PathBuf,
        version: ImgVersion,
        remove_existing: bool,
    ) -> Task<Message> {
        let index = self.editor.selected_archive().unwrap_or(0);
        let task = SaveTask::new(archive, path, version).remove_existing(remove_existing);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || task.run_blocking())
                    .await
                    .map_err(|e| format!("save task panicked: {e}"))?
                    .map_err(|e| e.to_string())
            },
            move |result| Message::SaveCompleted { index, result },
        )
    }

    fn scan_import_folder_task(
        index: usize,
        archive: ArchiveInfo,
        folder: PathBuf,
    ) -> Task<Message> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scan_import_folder(&folder, &archive))
                    .await
                    .map_err(|error| format!("folder scan task panicked: {error}"))?
                    .map_err(|error| error.to_string())
            },
            move |result| Message::FolderScanCompleted { index, result },
        )
    }

    fn run_folder_import(
        &self,
        index: usize,
        archive: ArchiveInfo,
        plan: FolderImportPlan,
        duplicate_policy: FolderDuplicatePolicy,
    ) -> Task<Message> {
        let task = FolderImportTask::new(archive, plan, duplicate_policy);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || task.run_blocking())
                    .await
                    .map_err(|error| format!("folder import task panicked: {error}"))?
                    .map_err(|error| error.to_string())
            },
            move |result| Message::FolderImportCompleted { index, result },
        )
    }

    fn folder_import_target_matches(
        &self,
        index: usize,
        target_name: &str,
        target_path: Option<&PathBuf>,
    ) -> bool {
        self.editor.archives().get(index).is_some_and(|archive| {
            archive.file_name == target_name && archive.path.as_ref() == target_path
        })
    }

    fn run_pack(&self, archive: ArchiveInfo, path: PathBuf, version: ImgVersion) -> Task<Message> {
        let index = self.editor.selected_archive().unwrap_or(0);
        let task = PackTask::new(archive, path, version);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || task.run_blocking())
                    .await
                    .map_err(|e| format!("pack task panicked: {e}"))?
                    .map_err(|e| e.to_string())
            },
            move |result| Message::PackCompleted { index, result },
        )
    }

    fn handle_shortcut(&mut self, shortcut: Shortcut) -> Task<Message> {
        match shortcut {
            Shortcut::New => Task::done(Message::NewArchive),
            Shortcut::Open => Task::done(Message::OpenArchive),
            Shortcut::Save => Task::done(Message::SaveArchive),
            Shortcut::SaveAs => Task::done(Message::SaveArchiveAs),
            Shortcut::Close => Task::done(Message::CloseSelectedArchive),
            Shortcut::Import => Task::done(Message::ImportFiles),
            Shortcut::ImportReplace => Task::done(Message::ImportFiles),
            Shortcut::ExportAll => Task::done(Message::ExportAll),
            Shortcut::ExportSelected => Task::done(Message::ExportSelected),
            Shortcut::SelectAll => Task::done(Message::SelectAll),
            Shortcut::InvertSelection => Task::done(Message::InvertSelection),
            Shortcut::ClearSelection => Task::done(Message::ClearSelection),
            Shortcut::Delete => Task::done(Message::DeleteSelected),
            Shortcut::FocusSearch => {
                if !self.config.show_search_bar {
                    self.config.show_search_bar = true;
                    self.save_config();
                }
                self.search_focused = true;
                iced::widget::operation::focus(iced::widget::Id::new(SEARCH_INPUT_ID))
            }
            Shortcut::CheckUpdates => Task::done(Message::CheckUpdatesManual),
            Shortcut::SwitchTab(tab) => Task::done(Message::Viewer3dSelectTab(tab)),
        }
    }
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),

            Message::ShortcutPressed(shortcut) => self.begin_shortcut_focus_check(shortcut),

            Message::NewArchive => {
                self.editor.new_archive();
                self.active_viewer_entry = None;
                self.clear_viewer_load();
                self.viewer3d_handle.clear();
                Task::none()
            }

            Message::OpenArchive => {
                self.toast = None;
                dialogs::open_file().map(Message::OpenArchiveResult)
            }

            Message::OpenArchiveResult(Some(path)) => self.open_archive_path(path),
            Message::OpenArchiveResult(None) => Task::none(),
            Message::ArchiveOpenCompleted { path, outcome } => {
                match outcome {
                    OpenArchiveOutcome::Opened(archive) => {
                        if self.editor.add_opened_archive(*archive) {
                            if let Some(opened) = self.editor.archives_mut().last_mut() {
                                Self::adopt_target(&self.config, opened);
                            }
                            self.config.recent_files.touch(&path);
                            self.save_config();
                        } else {
                            self.toast =
                                Some(format!("Already open: {}", path.display()));
                        }
                    }
                    OpenArchiveOutcome::Unsupported => {
                        self.show_unsupported = Some(path);
                    }
                    OpenArchiveOutcome::Failed(error) => {
                        self.toast = Some(format!("Failed to open archive: {error}"));
                    }
                }
                Task::none()
            }

            Message::OpenRecent(path) => {
                // The menu only emits paths that still exist on disk
                // (RecentFiles::iter_existing), but the file may have
                // been deleted between menu render and click. Guard
                // anyway so we don't surprise the user with an
                // "unsupported format" toast.
                if !path.exists() {
                    self.config.recent_files.remove(&path);
                    self.save_config();
                    self.toast = Some(format!("File no longer exists: {}", path.display()));
                    return Task::none();
                }
                self.open_archive_path(path)
            }

            Message::SaveArchive => {
                self.toast = None;
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let Some(path) = archive.path.clone() else {
                    return Task::done(Message::SaveArchiveAs);
                };
                if !path.exists() {
                    return Task::done(Message::SaveArchiveAs);
                }
                let version = archive.version;
                self.begin_save(archive, path, version, false)
            }

            Message::SaveArchiveAs => {
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let default_path = archive
                    .path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(format!("{}.img", archive.file_name)));
                let version = archive.version;
                dialogs::save_archive(default_path, version).map(Message::SaveArchiveAsResult)
            }

            Message::SaveArchiveAsResult(Some(choice)) => {
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                self.begin_save(archive, choice.path, choice.version, true)
            }
            Message::SaveArchiveAsResult(None) => Task::none(),

            Message::SaveCheckConfirmed => {
                let Some(pending) = self.pending_save.take() else {
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(pending.index).cloned() else {
                    self.toast = Some("The archive is no longer open.".into());
                    return Task::none();
                };
                if pending.fix
                    && pending
                        .issue
                        .as_ref()
                        .is_some_and(|issue| issue.has_fixable())
                {
                    // Plan and patch on a background task so a big
                    // archive never freezes the dialog.
                    let index = pending.index;
                    let path = pending.path;
                    let version = pending.version;
                    let remove_existing = pending.remove_existing;
                    self.toast = None;
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                crate::compat::normalize::collect_fixes(&archive)
                            })
                            .await
                            .unwrap_or_default()
                        },
                        move |patches| Message::SaveFixesReady {
                            index,
                            patches: SavePatches(patches),
                            path,
                            version,
                            remove_existing,
                        },
                    );
                }
                self.run_save(archive, pending.path, pending.version, pending.remove_existing)
            }
            Message::SaveCheckFixToggled(fix) => {
                if let Some(pending) = self.pending_save.as_mut() {
                    pending.fix = fix;
                }
                Task::none()
            }
            Message::SaveFixesReady {
                index,
                patches,
                path,
                version,
                remove_existing,
            } => {
                let patched = patches.len();
                let Some(archive) = self.editor.archives_mut().get_mut(index) else {
                    self.toast = Some("The archive is no longer open.".into());
                    return Task::none();
                };
                for (entry_index, bytes) in patches.0 {
                    if let Some(entry) = archive.entries.get_mut(entry_index) {
                        entry.override_bytes = Some(bytes);
                    }
                }
                if patched > 0 {
                    archive.dirty = true;
                    archive.invalidate_entry_caches_keeping_report();
                    self.toast = Some(format!(
                        "Repaired {patched} texture header(s); saving."
                    ));
                }
                let archive = archive.clone();
                self.run_save(archive, path, version, remove_existing)
            }
            Message::TextureReplaceRequested => {
                // Ignore a second request while one is already in flight.
                if self.replace_request.is_some() || self.replace_plan_in_flight {
                    self.toast =
                        Some("A replacement is already being prepared.".into());
                    return Task::none();
                }
                let Some(archive_index) = self.editor.selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let Some(entry_index) = self.editor.selected_entry() else {
                    self.toast = Some("Select a texture entry first.".into());
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(archive_index) else {
                    return Task::none();
                };
                if !archive.entries[entry_index].file_name_lower.ends_with(".txd") {
                    self.toast =
                        Some("Replacement works on TXD entries; that entry is not one.".into());
                    return Task::none();
                }
                let Some(target_id) = archive.target_game else {
                    self.toast = Some(
                        "No target set for this archive. Pick the game it is for in Validate textures."
                            .into(),
                    );
                    return self.open_validator_popup();
                };
                if let Err(error) = crate::compat::convert::writable_target(target_id) {
                    self.toast = Some(format!(
                        "{error} Pick the game this archive is for in Validate textures."
                    ));
                    return self.open_validator_popup();
                }
                self.replace_request = Some((archive_index, entry_index));
                self.replace_attempt = self.replace_attempt.wrapping_add(1);
                let attempt = self.replace_attempt;
                dialogs::pick_image_file()
                    .map(move |path| Message::ReplaceImagePicked { attempt, path })
            }
            Message::ReplaceImagePicked { attempt, path } => {
                if attempt != self.replace_attempt {
                    return Task::none();
                }
                let Some(path) = path else {
                    // A cancelled picker must not leave the flow armed.
                    self.replace_request = None;
                    return Task::none();
                };
                let Some((archive_index, entry_index)) = self.replace_request.take() else {
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(archive_index) else {
                    return Task::none();
                };
                let Some(target_id) = archive.target_game else {
                    return Task::none();
                };
                let Ok(target) = crate::compat::convert::writable_target(target_id) else {
                    return Task::none();
                };
                let entry = archive.entries[entry_index].clone();
                let archive_path = archive.path.clone();
                let archive_name = archive.file_name.clone();
                let texture_index = self.selected_texture;
                self.replace_plan_in_flight = true;
                self.toast = Some("Preparing replacement...".into());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_replace(
                                &entry,
                                archive_path.as_deref(),
                                archive_index,
                                entry_index,
                                texture_index,
                                path,
                                target,
                                &archive_name,
                                None,
                                crate::compat::encode::EncodeOptions::default(),
                            )
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::ReplacePlanned {
                        attempt,
                        result: Box::new(result),
                    },
                )
            }
            Message::ReplacePlanned { attempt, result } => {
                if attempt != self.replace_attempt {
                    return Task::none();
                }
                self.replace_plan_in_flight = false;
                let ready = match *result {
                    Ok(ready) => ready,
                    Err(error) => {
                        self.toast = Some(error);
                        return Task::none();
                    }
                };
                let before_handle = ready
                    .before
                    .as_ref()
                    .map(|(width, height, rgba)| {
                        iced::widget::image::Handle::from_rgba(*width, *height, rgba.as_ref().clone())
                    })
                    .unwrap_or_else(|| {
                        iced::widget::image::Handle::from_rgba(1, 1, vec![0, 0, 0, 0])
                    });
                let after_handle = iced::widget::image::Handle::from_rgba(
                    ready.plan.0.width,
                    ready.plan.0.height,
                    ready.plan.0.preview_rgba.clone(),
                );
                self.pending_replace = Some(ReplaceState {
                    archive_index: ready.archive_index,
                    entry_index: ready.entry_index,
                    texture_index: ready.texture_index,
                    source_path: ready.source_path,
                    source_name: ready.source_name,
                    texture_name: ready.texture_name,
                    entry: ready.entry,
                    archive_path: ready.archive_path,
                    target: ready.target,
                    archive_name: ready.archive_name,
                    chooser: ready.plan.0.format,
                    high_quality: false,
                    plan: ready.plan,
                    before_handle,
                    after_handle,
                    planning: false,
                });
                self.toast = None;
                Task::none()
            }
            Message::ReplaceFormatChanged(format) => {
                let Some(state) = self.pending_replace.as_mut() else {
                    return Task::none();
                };
                state.chooser = format;
                state.planning = true;
                // A fresh attempt invalidates any completion still in
                // flight from an earlier toggle.
                self.replace_attempt = self.replace_attempt.wrapping_add(1);
                let attempt = self.replace_attempt;
                let archive_index = state.archive_index;
                let entry_index = state.entry_index;
                let texture_index = state.texture_index;
                let path = state.source_path.clone();
                let entry = state.entry.clone();
                let archive_path = state.archive_path.clone();
                let target = state.target;
                let archive_name = state.archive_name.clone();
                let high_quality = state.high_quality;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_replace(
                                &entry,
                                archive_path.as_deref(),
                                archive_index,
                                entry_index,
                                texture_index,
                                path,
                                target,
                                &archive_name,
                                Some(format),
                                replace_encode_options(high_quality),
                            )
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::ReplacePlanRefreshed {
                        attempt,
                        result: Box::new(result),
                    },
                )
            }
            Message::ReplaceHighQualityToggled(high_quality) => {
                let Some(state) = self.pending_replace.as_mut() else {
                    return Task::none();
                };
                state.high_quality = high_quality;
                state.planning = true;
                // A fresh attempt invalidates any completion still in
                // flight from an earlier toggle.
                self.replace_attempt = self.replace_attempt.wrapping_add(1);
                let attempt = self.replace_attempt;
                let archive_index = state.archive_index;
                let entry_index = state.entry_index;
                let texture_index = state.texture_index;
                let path = state.source_path.clone();
                let entry = state.entry.clone();
                let archive_path = state.archive_path.clone();
                let target = state.target;
                let archive_name = state.archive_name.clone();
                let format = state.chooser;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_replace(
                                &entry,
                                archive_path.as_deref(),
                                archive_index,
                                entry_index,
                                texture_index,
                                path,
                                target,
                                &archive_name,
                                Some(format),
                                replace_encode_options(high_quality),
                            )
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::ReplacePlanRefreshed {
                        attempt,
                        result: Box::new(result),
                    },
                )
            }
            Message::ReplacePlanRefreshed {
                attempt,
                result,
            } => {
                if attempt != self.replace_attempt {
                    return Task::none();
                }
                let ready = match *result {
                    Ok(ready) => ready,
                    Err(error) => {
                        if let Some(state) = self.pending_replace.as_mut() {
                            state.planning = false;
                        }
                        self.toast = Some(error);
                        return Task::none();
                    }
                };
                if let Some(state) = self.pending_replace.as_mut() {
                    state.chooser = ready.plan.0.format;
                    state.after_handle = iced::widget::image::Handle::from_rgba(
                        ready.plan.0.width,
                        ready.plan.0.height,
                        ready.plan.0.preview_rgba.clone(),
                    );
                    state.plan = ready.plan;
                    state.planning = false;
                }
                Task::none()
            }
            Message::ReplaceConfirmed => {
                let Some(state) = self.pending_replace.take() else {
                    return Task::none();
                };
                // Invalidate any re-plan still in flight.
                self.replace_attempt = self.replace_attempt.wrapping_add(1);
                let archive_index = state.archive_index;
                let entry_index = state.entry_index;
                let texture_index = state.texture_index;
                let entry = state.entry.clone();
                let archive_path = state.archive_path.clone();
                let plan = state.plan.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || -> Result<Arc<Vec<u8>>, String> {
                            let bytes = crate::parser::read_entry_data_from_source(
                                &entry,
                                archive_path.as_deref(),
                            )
                            .map_err(|error| error.to_string())?;
                            let converted = crate::compat::convert::apply_replace(
                                &bytes,
                                texture_index,
                                &plan.0,
                            )?;
                            Ok(Arc::new(converted))
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::ReplaceApplied {
                        archive_index,
                        entry_index,
                        result: result.map(AppliedBytes),
                    },
                )
            }
            Message::ReplaceApplied {
                archive_index,
                entry_index,
                result,
            } => {
                match result {
                    Ok(AppliedBytes(bytes)) => {
                        let Some(archive) = self.editor.archives_mut().get_mut(archive_index)
                        else {
                            self.toast = Some("The archive is no longer open.".into());
                            return Task::none();
                        };
                        if let Some(entry) = archive.entries.get_mut(entry_index) {
                            entry.override_bytes = Some(bytes);
                        }
                        archive.dirty = true;
                        archive.invalidate_entry_caches_keeping_report();
                        self.toast = Some("Texture replaced - save the archive to write it.".into());
                        return self.decode_texture_entry(entry_index);
                    }
                    Err(error) => {
                        self.toast = Some(format!("Replace failed: {error}"));
                    }
                }
                Task::none()
            }
            Message::ReplaceCancelled => {
                self.replace_attempt = self.replace_attempt.wrapping_add(1);
                self.pending_replace = None;
                Task::none()
            }
            Message::ImportImageAsTxdRequested => {
                let Some(archive_index) = self.editor.selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(archive_index) else {
                    return Task::none();
                };
                let Some(target_id) = archive.target_game else {
                    self.toast = Some(
                        "No target set for this archive. Pick the game it is for in Validate textures."
                            .into(),
                    );
                    return self.open_validator_popup();
                };
                if let Err(error) = crate::compat::convert::writable_target(target_id) {
                    self.toast = Some(format!(
                        "{error} Pick the game this archive is for in Validate textures."
                    ));
                    return self.open_validator_popup();
                }
                if self.new_txd_picker_open || self.new_txd_plan_in_flight {
                    self.toast = Some("An import is already being prepared.".into());
                    return Task::none();
                }
                self.new_txd_picker_open = true;
                self.new_txd_attempt = self.new_txd_attempt.wrapping_add(1);
                let attempt = self.new_txd_attempt;
                dialogs::pick_image_file()
                    .map(move |path| Message::NewTxdImagePicked { attempt, path })
            }
            Message::NewTxdImagePicked { attempt, path } => {
                if attempt != self.new_txd_attempt {
                    return Task::none();
                }
                self.new_txd_picker_open = false;
                let Some(path) = path else {
                    return Task::none();
                };
                let Some(archive_index) = self.editor.selected_archive() else {
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(archive_index) else {
                    return Task::none();
                };
                let Some(target_id) = archive.target_game else {
                    return Task::none();
                };
                let Ok(target) = crate::compat::convert::writable_target(target_id) else {
                    return Task::none();
                };
                let archive_name = archive.file_name.clone();
                self.new_txd_plan_in_flight = true;
                self.toast = Some("Preparing import...".into());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_txd_import(
                                archive_index,
                                path,
                                target,
                                &archive_name,
                                None,
                                crate::compat::encode::EncodeOptions::default(),
                            )
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::NewTxdPlanned {
                        attempt,
                        result: Box::new(result),
                    },
                )
            }
            Message::NewTxdPlanned { attempt, result } => {
                if attempt != self.new_txd_attempt {
                    return Task::none();
                }
                self.new_txd_plan_in_flight = false;
                let ready = match *result {
                    Ok(ready) => ready,
                    Err(error) => {
                        self.toast = Some(error);
                        return Task::none();
                    }
                };
                // Keep whatever name and quality choice the user already
                // set across a re-plan.
                let texture_name = self
                    .pending_new_txd
                    .as_ref()
                    .map(|state| state.texture_name.clone())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| ready.texture_name.clone());
                let high_quality = self
                    .pending_new_txd
                    .as_ref()
                    .is_some_and(|state| state.high_quality);
                let after_handle = iced::widget::image::Handle::from_rgba(
                    ready.plan.0.width,
                    ready.plan.0.height,
                    ready.plan.0.preview_rgba.clone(),
                );
                self.pending_new_txd = Some(NewTxdState {
                    archive_index: ready.archive_index,
                    source_path: ready.source_path,
                    source_name: ready.source_name,
                    texture_name,
                    target: ready.target,
                    chooser: ready.plan.0.format,
                    high_quality,
                    plan: ready.plan,
                    after_handle,
                    planning: false,
                });
                self.toast = None;
                Task::none()
            }
            Message::NewTxdNameChanged(name) => {
                if let Some(state) = self.pending_new_txd.as_mut() {
                    state.texture_name = name;
                }
                Task::none()
            }
            Message::NewTxdFormatChanged(format) => {
                let Some(state) = self.pending_new_txd.as_mut() else {
                    return Task::none();
                };
                state.chooser = format;
                state.planning = true;
                self.new_txd_attempt = self.new_txd_attempt.wrapping_add(1);
                let attempt = self.new_txd_attempt;
                let archive_index = state.archive_index;
                let path = state.source_path.clone();
                let target = state.target;
                let high_quality = state.high_quality;
                let archive_name = self
                    .editor
                    .archives()
                    .get(archive_index)
                    .map(|archive| archive.file_name.clone())
                    .unwrap_or_default();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_txd_import(
                                archive_index,
                                path,
                                target,
                                &archive_name,
                                Some(format),
                                replace_encode_options(high_quality),
                            )
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::NewTxdPlanned {
                        attempt,
                        result: Box::new(result),
                    },
                )
            }
            Message::NewTxdHighQualityToggled(high_quality) => {
                let Some(state) = self.pending_new_txd.as_mut() else {
                    return Task::none();
                };
                state.high_quality = high_quality;
                state.planning = true;
                self.new_txd_attempt = self.new_txd_attempt.wrapping_add(1);
                let attempt = self.new_txd_attempt;
                let archive_index = state.archive_index;
                let path = state.source_path.clone();
                let target = state.target;
                let format = state.chooser;
                let archive_name = self
                    .editor
                    .archives()
                    .get(archive_index)
                    .map(|archive| archive.file_name.clone())
                    .unwrap_or_default();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_txd_import(
                                archive_index,
                                path,
                                target,
                                &archive_name,
                                Some(format),
                                replace_encode_options(high_quality),
                            )
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::NewTxdPlanned {
                        attempt,
                        result: Box::new(result),
                    },
                )
            }
            Message::NewTxdConfirmed => {
                let Some(state) = self.pending_new_txd.take() else {
                    return Task::none();
                };
                let mut name = state.texture_name.trim().to_string();
                if name.is_empty() {
                    self.toast = Some("Give the new TXD a name.".into());
                    self.pending_new_txd = Some(state);
                    return Task::none();
                }
                if !name.to_ascii_lowercase().ends_with(".txd") {
                    name.push_str(".txd");
                }
                let bytes = crate::compat::convert::build_new_txd(&state.plan.0, state.target, &name);
                let Some(archive) = self.editor.archives_mut().get_mut(state.archive_index) else {
                    self.toast = Some("The archive is no longer open.".into());
                    return Task::none();
                };
                if archive
                    .entries
                    .iter()
                    .any(|entry| entry.file_name.eq_ignore_ascii_case(&name))
                {
                    self.toast = Some(format!("An entry named '{name}' already exists."));
                    self.pending_new_txd = Some(state);
                    return Task::none();
                }
                // The entry is in: invalidate any plan still in flight so
                // it cannot re-open the dialog.
                self.new_txd_attempt = self.new_txd_attempt.wrapping_add(1);
                let mut entry = crate::archive::EntryInfo::new(&name);
                entry.imported = true;
                entry.override_bytes = Some(Arc::new(bytes));
                let new_index = archive.entries.len();
                archive.entries.push(entry);
                archive.dirty = true;
                archive.invalidate_entry_caches_keeping_report();
                let archive_index = state.archive_index;
                self.toast = Some(format!(
                    "Added '{name}' - save the archive to write it.",
                ));
                if let Some(archive) = self.editor.archives_mut().get_mut(archive_index) {
                    for entry in archive.entries.iter_mut() {
                        entry.selected = false;
                    }
                    if let Some(entry) = archive.entries.get_mut(new_index) {
                        entry.selected = true;
                    }
                    archive.update_selected_list(&self.search, false);
                }
                self.editor.select_entry(new_index, false, false);
                self.refresh_imported_verdicts(archive_index);
                Task::none()
            }
            Message::NewTxdCancelled => {
                self.new_txd_attempt = self.new_txd_attempt.wrapping_add(1);
                self.pending_new_txd = None;
                Task::none()
            }
            Message::BulkConvertRequested => {
                let Some(archive_index) = self.editor.selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(archive_index) else {
                    return Task::none();
                };
                let Some(target_id) = archive.target_game else {
                    self.toast = Some("Set a game target first (Validate textures).".into());
                    return Task::none();
                };
                let Ok(target) = crate::compat::convert::writable_target(target_id) else {
                    self.toast = Some("Bully (Gamebryo) texture writing is not supported yet.".into());
                    return Task::none();
                };
                let selected: Vec<usize> = archive.selected_indices.iter().copied().collect();
                if selected.is_empty() {
                    self.toast = Some("Select the entries to convert first.".into());
                    return Task::none();
                }
                let archive = archive.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            plan_bulk_convert(archive_index, &archive, &selected, target)
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    |result| Message::BulkConvertPlanned(Box::new(result)),
                )
            }
            Message::BulkConvertPlanned(result) => {
                let ready = match *result {
                    Ok(ready) => ready,
                    Err(error) => {
                        self.toast = Some(error);
                        return Task::none();
                    }
                };
                if ready.entries.is_empty() {
                    self.toast = Some("Every selected texture is already native for the target.".into());
                    return Task::none();
                }
                self.pending_bulk = Some(BulkConvertState {
                    archive_index: ready.archive_index,
                    source_label: ready.source_label,
                    entries: ready.entries,
                });
                self.toast = None;
                Task::none()
            }
            Message::BulkConvertConfirmed => {
                let Some(state) = self.pending_bulk.take() else {
                    return Task::none();
                };
                let archive_index = state.archive_index;
                let Some(archive) = self.editor.archives().get(archive_index).cloned() else {
                    self.toast = Some("The archive is no longer open.".into());
                    return Task::none();
                };
                let entries = state.entries;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            convert_bulk_entries(&archive, &entries)
                        })
                        .await
                        .unwrap_or_else(|error| Err(format!("task panicked: {error}")))
                    },
                    move |result| Message::BulkConvertApplied {
                        archive_index,
                        result: result.map(SavePatches),
                    },
                )
            }
            Message::BulkConvertApplied {
                archive_index,
                result,
            } => {
                match result {
                    Ok(SavePatches(patches)) => {
                        let texture_count: usize = patches.len();
                        let Some(archive) = self.editor.archives_mut().get_mut(archive_index)
                        else {
                            self.toast = Some("The archive is no longer open.".into());
                            return Task::none();
                        };
                        for (entry_index, bytes) in patches {
                            if let Some(entry) = archive.entries.get_mut(entry_index) {
                                entry.override_bytes = Some(bytes);
                            }
                        }
                        archive.dirty = true;
                        archive.invalidate_entry_caches_keeping_report();
                        self.toast = Some(format!(
                            "Converted {texture_count} entries - save the archive to write them.",
                        ));
                        let _ = self.refresh_inspection();
                    }
                    Err(error) => {
                        self.toast = Some(format!("Conversion failed: {error}"));
                    }
                }
                Task::none()
            }
            Message::BulkConvertCancelled => {
                self.pending_bulk = None;
                Task::none()
            }
            Message::SaveCheckCancelled => {
                self.pending_save = None;
                // A guard-initiated save that got cancelled cancels the
                // whole close request.
                self.close_after_save = None;
                self.toast = Some("Save cancelled.".into());
                Task::none()
            }

            Message::SaveCompleted { index, result } => {
                match result {
                    Ok(archive) => {
                        self.editor.replace_archive(index, archive);
                        if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                            Self::adopt_target(&self.config, archive);
                        }
                        self.toast = Some("Archive saved.".into());
                        // The unsaved-changes guard may have saved in
                        // order to close: finish the close now.
                        if self.close_after_save == Some(index) {
                            self.close_after_save = None;
                            return self.close_archive_at(index);
                        }
                    }
                    Err(err) => {
                        self.close_after_save = None;
                        self.toast = Some(format!("Save failed: {err}"));
                    }
                };
                Task::none()
            }
            Message::PackArchive => {
                self.toast = None;
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                if archive.progress.in_use() {
                    self.toast = Some("An archive operation is already running.".into());
                    return Task::none();
                }
                let Some(path) = archive.path.clone() else {
                    self.toast = Some("Save the archive before packing it.".into());
                    return Task::none();
                };
                if !path.exists() {
                    self.toast =
                        Some("The archive file no longer exists. Use Save as… first.".into());
                    return Task::none();
                }
                let version = archive.version;
                self.run_pack(archive, path, version)
            }
            Message::PackCompleted { index, result } => {
                match result {
                    Ok(outcome) => {
                        let reclaimed = outcome.stats.reclaimed_bytes();
                        let packed = outcome.stats.packed_bytes;
                        self.editor.replace_archive(index, outcome.archive);
                        self.toast = if reclaimed > 0 {
                            Some(format!(
                                "Archive packed — reclaimed {} ({} on disk).",
                                format_byte_count(reclaimed),
                                format_byte_count(packed)
                            ))
                        } else {
                            Some(format!(
                                "Archive packed — no space reclaimed ({} on disk).",
                                format_byte_count(packed)
                            ))
                        };
                    }
                    Err(err) => {
                        self.toast = Some(format!("Pack failed: {err}"));
                    }
                }
                Task::none()
            }
            _ => self.update_tail(message),
        }
    }

    fn update_tail(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop
            | Message::ShortcutPressed(_)
            | Message::NewArchive
            | Message::OpenArchive
            | Message::OpenArchiveResult(_)
            | Message::ArchiveOpenCompleted { .. }
            | Message::OpenRecent(_)
            | Message::SaveArchive
            | Message::SaveArchiveAs
            | Message::SaveArchiveAsResult(_)
            | Message::SaveCompleted { .. }
            | Message::SaveCheckConfirmed
            | Message::SaveCheckFixToggled(_)
            | Message::SaveFixesReady { .. }
            | Message::SaveCheckCancelled
            | Message::TextureReplaceRequested
            | Message::ReplaceImagePicked { .. }
            | Message::ReplacePlanned { .. }
            | Message::ReplaceFormatChanged(_)
            | Message::ReplaceHighQualityToggled(_)
            | Message::ReplacePlanRefreshed { .. }
            | Message::ReplaceConfirmed
            | Message::ReplaceApplied { .. }
            | Message::ReplaceCancelled
            | Message::ImportImageAsTxdRequested
            | Message::NewTxdImagePicked { .. }
            | Message::NewTxdPlanned { .. }
            | Message::NewTxdNameChanged(_)
            | Message::NewTxdFormatChanged(_)
            | Message::NewTxdHighQualityToggled(_)
            | Message::NewTxdConfirmed
            | Message::NewTxdCancelled
            | Message::BulkConvertRequested
            | Message::BulkConvertPlanned(_)
            | Message::BulkConvertConfirmed
            | Message::BulkConvertApplied { .. }
            | Message::BulkConvertCancelled
            | Message::PackArchive
            | Message::PackCompleted { .. } => Task::none(),

            Message::CloseSelectedArchive => {
                let Some(index) = self.editor.selected_archive() else {
                    return Task::none();
                };
                self.request_archive_close(index)
            }
            Message::CloseArchiveTab(index) => self.request_archive_close(index),
            Message::WindowCloseRequested(window) => {
                if self.editor.archives().iter().any(|archive| archive.dirty) {
                    self.pending_close = Some(PendingClose::Window(window));
                    Task::none()
                } else {
                    self.hide_window_then_close(window)
                }
            }
            Message::CloseGuardSave => {
                let Some(pending) = self.pending_close else {
                    return Task::none();
                };
                match pending {
                    PendingClose::Archive(index) => {
                        let Some(archive) = self.editor.archives().get(index).cloned() else {
                            self.pending_close = None;
                            return Task::none();
                        };
                        self.pending_close = None;
                        self.close_after_save = Some(index);
                        // Route through the normal save (and its pre-save
                        // report when one is needed).
                        let Some(path) = archive.path.clone() else {
                            return Task::done(Message::SaveArchiveAs);
                        };
                        if !path.exists() {
                            return Task::done(Message::SaveArchiveAs);
                        }
                        let version = archive.version;
                        self.begin_save(archive, path, version, false)
                    }
                    PendingClose::Window(_) => {
                        // Quitting saves nothing by itself; the dialog
                        // only offers discard for the whole window.
                        Task::none()
                    }
                }
            }
            Message::CloseGuardDiscard => {
                let Some(pending) = self.pending_close.take() else {
                    return Task::none();
                };
                match pending {
                    PendingClose::Archive(index) => self.close_archive_at(index),
                    PendingClose::Window(window) => {
                        // Fade to black before closing: the last visible
                        // frame must be dark, so swapchain/DWM teardown
                        // cannot flash the (light) desktop buffer at
                        // low-light users. The deadline guarantees the
                        // close even if the fade is interrupted.
                        self.quitting = Some(window);
                        self.animator.animate_from_current(
                            ANIM_QUIT_FADE,
                            1.0,
                            QUIT_FADE_DURATION,
                            crate::ui::easing::Easing::CubicOut,
                        );
                        Task::perform(
                            async {
                                tokio::time::sleep(QUIT_FADE_DEADLINE).await;
                            },
                            |_| Message::QuitFadeDone,
                        )
                    }
                }
            }
            Message::QuitFadeDone => {
                if let Some(window) = self.quitting {
                    // Keep `quitting` set until the window is hidden. Clearing
                    // it here can produce one unmasked frame at the exact
                    // point where the compositor is being torn down.
                    self.hide_window_then_close(window)
                } else {
                    Task::none()
                }
            }
            Message::QuitWindowHidden => {
                if let Some(window) = self.quitting.take() {
                    iced::window::close::<Message>(window).map(|_| Message::Noop)
                } else {
                    Task::none()
                }
            }
            Message::CloseGuardCancel => {
                self.pending_close = None;
                self.close_after_save = None;
                Task::none()
            }
            Message::SelectArchiveTab(index) => {
                self.start_archive_tab_feedback(index);
                self.start_click_ripple(RippleTarget::ArchiveTab(index));
                self.editor.select_archive(index);
                self.active_viewer_entry = None;
                self.clear_viewer_load();
                self.viewer3d_handle.clear();
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }

            Message::ImportFiles => {
                self.toast = None;
                dialogs::import_files().map(Message::ImportFilesResult)
            }
            Message::ImportFilesResult(paths) => {
                if paths.is_empty() {
                    return Task::none();
                }
                let Some((index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("Open an archive first to import into it.".into());
                    return Task::none();
                };
                self.begin_import(index, archive, paths, None)
            }
            Message::ImportPreflightCompleted {
                index,
                paths,
                folder,
                checks,
            } => {
                if checks.iter().any(|check| check.has_issues()) {
                    self.pending_import = Some(PendingImport {
                        index,
                        paths,
                        folder: folder.map(|boxed| *boxed),
                        checks,
                    });
                    return Task::none();
                }
                let Some(archive) = self.editor.archives().get(index).cloned() else {
                    return Task::none();
                };
                match folder.map(|boxed| *boxed) {
                    Some((plan, policy)) => self.run_folder_import(index, archive, plan, policy),
                    None => Self::import_archive_task(index, archive, paths, true),
                }
            }
            Message::ImportCheckConfirmed => {
                let Some(pending) = self.pending_import.take() else {
                    return Task::none();
                };
                let Some(archive) = self.editor.archives().get(pending.index).cloned() else {
                    self.toast = Some("The target archive is no longer open.".into());
                    return Task::none();
                };
                match pending.folder {
                    Some((plan, policy)) => {
                        self.run_folder_import(pending.index, archive, plan, policy)
                    }
                    None => Self::import_archive_task(pending.index, archive, pending.paths, true),
                }
            }
            Message::ImportCheckCancelled => {
                self.pending_import = None;
                self.toast = Some("Import cancelled.".into());
                Task::none()
            }
            Message::ImportCompleted {
                index,
                count,
                checked,
                result,
            } => {
                match result {
                    Ok(archive) => {
                        self.editor.replace_archive(index, archive);
                        if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                            archive.update_selected_list(&self.search, self.config.literal_file_types);
                        }
                        if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                            Self::adopt_target(&self.config, archive);
                        }
                        self.refresh_imported_verdicts(index);
                        self.toast = Some(if checked {
                            format!("Imported {count} files.")
                        } else {
                            format!(
                                "Imported {count} files - no validator target set, formats were not checked."
                            )
                        });
                    }
                    Err(error) => {
                        self.toast = Some(format!("Import failed: {error}"));
                    }
                }
                Task::none()
            }

            Message::ImportFolder => {
                self.toast = None;
                if self.editor.selected_archive().is_none() {
                    self.toast = Some("Open an archive first to import a folder.".into());
                    return Task::none();
                }
                dialogs::import_folder().map(Message::ImportFolderResult)
            }
            Message::ImportFolderResult(Some(folder)) => {
                let Some((index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("Open an archive first to import a folder.".into());
                    return Task::none();
                };
                Self::scan_import_folder_task(index, archive, folder)
            }
            Message::ImportFolderResult(None) => Task::none(),
            Message::FolderScanCompleted { index, result } => {
                match result {
                    Ok(plan) if plan.files.is_empty() => {
                        self.toast = Some(format!(
                            "No regular files found in {}.",
                            plan.folder.display()
                        ));
                    }
                    Ok(plan) => {
                        if self.folder_import_target_matches(
                            index,
                            &plan.target_archive_name,
                            plan.target_archive_path.as_ref(),
                        ) {
                            self.pending_folder_import = Some((index, plan));
                        } else {
                            self.toast = Some(
                                "The target archive changed while the folder was being scanned."
                                    .into(),
                            );
                        }
                    }
                    Err(error) => {
                        self.toast = Some(format!("Folder scan failed: {error}"));
                    }
                }
                Task::none()
            }
            Message::ConfirmFolderImport(duplicate_policy) => {
                let Some((index, plan)) = self.pending_folder_import.take() else {
                    return Task::none();
                };
                if !self.folder_import_target_matches(
                    index,
                    &plan.target_archive_name,
                    plan.target_archive_path.as_ref(),
                ) {
                    self.toast = Some("The target archive is no longer selected.".into());
                    return Task::none();
                }
                let Some(archive) = self.editor.archives().get(index).cloned() else {
                    self.toast = Some("The target archive is no longer open.".into());
                    return Task::none();
                };
                if archive.progress.in_use() {
                    self.toast = Some("An archive operation is already running.".into());
                    return Task::none();
                }
                self.begin_import(index, archive, plan.files.clone(), Some((plan, duplicate_policy)))
            }
            Message::CancelFolderImport => {
                self.pending_folder_import = None;
                Task::none()
            }
            Message::FolderImportCompleted { index, result } => {
                match result {
                    Ok(outcome) => {
                        if !self.folder_import_target_matches(
                            index,
                            &outcome.target_archive_name,
                            outcome.target_archive_path.as_ref(),
                        ) {
                            self.toast = Some(
                                "Folder import discarded because the target archive changed."
                                    .into(),
                            );
                            return Task::none();
                        }
                        let summary = outcome.summary;
                        self.editor.replace_archive(index, outcome.archive);
                        if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                            archive.update_selected_list(&self.search, self.config.literal_file_types);
                        }
                        self.refresh_imported_verdicts(index);
                        self.toast = Some(format_folder_import_summary(&summary));
                    }
                    Err(error) => {
                        self.toast = Some(format!("Folder import failed: {error}"));
                    }
                }
                Task::none()
            }

            Message::ExportAll => self.start_export(ExportMode::All),
            Message::ExportSelected => self.start_export(ExportMode::Selected),

            Message::ExportFolderResult(Some(folder)) => {
                let Some((index, archive)) = self.editor.clone_selected_archive() else {
                    return Task::none();
                };
                let mode = if self.last_export_selected_only {
                    ExportMode::Selected
                } else {
                    ExportMode::All
                };
                self.last_export_selected_only = false;
                self.config.last_export_folder = Some(folder.clone());
                self.save_config();
                if let Some(archive) = self.editor.selected_archive_mut() {
                    archive.last_export_folder = Some(folder.clone());
                }
                let task = ExportTask::new(archive, folder, mode);
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || task.run_blocking())
                            .await
                            .map_err(|e| format!("export task panicked: {e}"))?
                            .map_err(|e| e.to_string())
                    },
                    move |result| Message::ExportCompleted { index, result },
                )
            }
            Message::ExportFolderResult(None) => Task::none(),

            Message::ExportCompleted { index, result } => {
                if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                    match result {
                        Ok((count, names)) => {
                            archive.export_status = ExportStatus::Done;
                            archive.last_export_count = count;
                            let now = chrono::Local::now().format("%H:%M:%S");
                            let summary = if count == 1 {
                                names
                                    .first()
                                    .cloned()
                                    .unwrap_or_else(|| "1 file".to_string())
                            } else {
                                format!("{count} files")
                            };
                            archive
                                .recent_exports
                                .push(format!("[{now}] Exported {summary}"));
                            archive.add_log(format!("Exported {count} entries"));
                            self.toast = Some(format!("Exported {count} entries."));
                        }
                        Err(err) => {
                            archive.export_status = ExportStatus::Idle;
                            archive.last_export_count = 0;
                            archive.add_log(format!("Export failed: {err}"));
                            self.toast = Some(format!("Export failed: {err}"));
                        }
                    }
                }
                Task::none()
            }

            Message::SelectAll => {
                self.editor.select_all(true);
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::InvertSelection => {
                self.editor.invert_selection();
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::ClearSelection => {
                // Escape also dismisses any pending dialog. This is the
                // safety net: a modal with a rendering bug must never be
                // able to trap the user (the window close is intercepted
                // while a dirty-archive guard is open).
                if self.pending_close.is_some() {
                    self.pending_close = None;
                    self.close_after_save = None;
                    return Task::none();
                }
                if self.pending_import.is_some() {
                    self.pending_import = None;
                    return Task::none();
                }
                if self.pending_save.is_some() {
                    self.pending_save = None;
                    self.close_after_save = None;
                    self.toast = Some("Save cancelled.".into());
                    return Task::none();
                }
                if self.pending_replace.is_some() {
                    self.pending_replace = None;
                    return Task::none();
                }
                if self.pending_new_txd.is_some() {
                    self.pending_new_txd = None;
                    return Task::none();
                }
                if self.pending_bulk.is_some() {
                    self.pending_bulk = None;
                    return Task::none();
                }
                self.editor.clear_selection();
                self.inspected_entry = None;
                self.reset_texture_preview_state();
                self.active_viewer_entry = None;
                self.clear_viewer_load();
                self.viewer3d_handle.clear();
                Task::none()
            }
            Message::DeleteSelected => {
                self.editor.delete_selected();
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::StartRename => {
                if let Some(index) = self.editor.selected_entry() {
                    if let Some(archive) = self
                        .editor
                        .archives()
                        .get(self.editor.selected_archive().unwrap_or(0))
                        && let Some(entry) = archive.entries.get(index)
                    {
                        self.rename_buffer = entry.file_name.to_string();
                    }
                    if let Some(archive) = self.editor.selected_archive_mut() {
                        archive.set_rename(index);
                    }
                    self.rename_focused = true;
                    return iced::widget::operation::focus(iced::widget::Id::new(RENAME_INPUT_ID));
                }
                Task::none()
            }
            Message::RenameInputChanged(value) => {
                self.rename_buffer = value;
                self.rename_focused = true;
                Task::none()
            }
            Message::CommitRename => {
                let new_name = self.rename_buffer.clone();
                self.editor.rename_selected(&new_name);
                self.rename_buffer.clear();
                self.rename_focused = false;
                Task::none()
            }
            Message::CancelRename => {
                if let Some(archive) = self.editor.selected_archive_mut() {
                    archive.clear_rename();
                }
                self.rename_buffer.clear();
                self.rename_focused = false;
                Task::none()
            }
            Message::CancelActive => {
                for archive in self.editor.archives_mut() {
                    if archive.progress.in_use() {
                        archive.progress.request_cancel();
                    }
                }
                Task::none()
            }

            Message::SearchChanged(value) => {
                // Update the bound search text immediately so the text_input
                // widget stays in sync with the user's keystrokes. The
                // expensive filter rebuild is deferred to DebounceTick.
                if value != self.search {
                    self.search = value;
                    self.filter_pending = true;
                }
                self.predictions_dismissed = false;
                self.prediction_index = None;
                self.search_focused = true;
                Task::none()
            }
            Message::ClearSearch => {
                self.search.clear();
                self.filter_pending = true;
                self.predictions_dismissed = false;
                self.prediction_index = None;
                self.search_focused = true;
                iced::widget::operation::focus(iced::widget::Id::new(SEARCH_INPUT_ID))
            }
            Message::SearchPredictMove(direction) => {
                if !self.predictions_open() {
                    return Task::none();
                }
                let count =
                    self.search_predictions.len() + usize::from(self.did_you_mean.is_some());
                if count == 0 {
                    return Task::none();
                }
                let current = match self.prediction_index {
                    Some(index) => index as i32,
                    // Down selects the first row, Up selects the last.
                    None => {
                        if direction < 0 {
                            count as i32
                        } else {
                            -1
                        }
                    }
                };
                let next = (current + direction).clamp(0, count as i32 - 1);
                self.prediction_index = Some(next as usize);
                Task::none()
            }
            Message::SearchPredictCommit => {
                if !self.predictions_open() {
                    return Task::none();
                }
                let match_count = self.search_predictions.len();
                let entry_index = match self.prediction_index {
                    Some(index) if index < match_count => Some(self.search_predictions[index].0),
                    Some(_) => self.did_you_mean.as_ref().map(|(entry, _)| *entry),
                    None => {
                        if match_count > 0 {
                            Some(self.search_predictions[0].0)
                        } else {
                            self.did_you_mean.as_ref().map(|(entry, _)| *entry)
                        }
                    }
                };
                match entry_index {
                    Some(entry_index) => self.commit_search_prediction(entry_index),
                    None => Task::none(),
                }
            }
            Message::SearchPredictDismiss => {
                self.predictions_dismissed = true;
                self.prediction_index = None;
                Task::none()
            }
            Message::SearchPredictPick(index) => {
                match self.search_predictions.get(index) {
                    Some(&(entry_index, _)) => self.commit_search_prediction(entry_index),
                    None => Task::none(),
                }
            }
            Message::SearchPickDidYouMean => {
                match self.did_you_mean.as_ref() {
                    Some(&(entry_index, _)) => self.commit_search_prediction(entry_index),
                    None => Task::none(),
                }
            }
            Message::UncapturedPress => {
                if self.predictions_open() {
                    self.close_predictions();
                }
                Task::none()
            }
            Message::FocusSearchInput => {
                self.search_focused = true;
                iced::widget::operation::focus(iced::widget::Id::new(SEARCH_INPUT_ID))
            }
            Message::SearchFocusChanged(focused) => {
                self.search_focused = focused;
                self.pending_search_focus = Some(focused);
                self.resolve_shortcut_focus_check()
            }
            Message::RenameFocusChanged(focused) => {
                self.rename_focused = focused;
                self.pending_rename_focus = Some(focused);
                self.resolve_shortcut_focus_check()
            }
            Message::DebounceTick => {
                if self.filter_pending {
                    self.filter_pending = false;
                    return self.run_refresh_filter();
                }
                Task::none()
            }
            Message::RefreshFilter => {
                self.editor.update_filtered_list(&self.search);
                Task::none()
            }

            Message::CopySelectedEntryDetails => {
                let Some((_, inspection)) = self.inspected_entry.as_ref() else {
                    return Task::none();
                };
                let mut lines = Vec::new();
                lines.push(format!("Name: {}", inspection.file_name));
                lines.push(format!("Type: {}", inspection.file_type));
                lines.push(format!(
                    "Size: {} bytes ({} sectors)",
                    inspection.size_bytes, inspection.size_sectors
                ));
                lines.push(format!(
                    "Offset: sector {} (byte {})",
                    inspection.offset_bytes / 2048,
                    inspection.offset_bytes
                ));
                lines.push(format!("Source: {}", inspection.source));
                for (key, value) in &inspection.summary {
                    lines.push(format!("{key}: {value}"));
                }
                let text = lines.join("\n");
                self.toast = Some("Copied selected entry details".to_string());
                iced::clipboard::write::<Message>(text)
            }
            Message::CopyLogs => {
                let Some(archive) = self
                    .editor
                    .archives()
                    .get(self.editor.selected_archive().unwrap_or(0))
                else {
                    return Task::none();
                };
                let text = archive.logs.join("\n");
                self.toast = Some("Copied logs".to_string());
                iced::clipboard::write::<Message>(text)
            }

            Message::EntryClicked(display_row) => {
                if self.predictions_open() {
                    self.close_predictions();
                }
                if let Some(entry_index) = self.display_row_to_entry(display_row) {
                    if let Some(archive_index) = self.editor.selected_archive() {
                        self.start_entry_feedback((archive_index, entry_index));
                        self.start_click_ripple(RippleTarget::Entry {
                            archive_index,
                            entry_index,
                        });
                    }
                    let shift = self.modifiers.shift();
                    let ctrl = self.modifiers.command();
                    self.editor.select_entry(entry_index, shift, ctrl);
                    self.clear_stale_viewer_load();
                    self.reset_texture_preview_state();
                    let inspection_task = self.refresh_inspection();
                    let preview_task = if shift || ctrl {
                        Task::none()
                    } else {
                        self.refresh_active_preview()
                    };
                    Task::batch(vec![inspection_task, preview_task])
                } else {
                    Task::none()
                }
            }
            Message::EntryDoubleClicked(display_row) => {
                let task = if let Some(entry_index) = self.display_row_to_entry(display_row) {
                    self.editor.set_selected_entry(Some(entry_index));
                    self.editor.select_entry(entry_index, false, false);
                    self.clear_stale_viewer_load();
                    self.reset_texture_preview_state();
                    if let Some(archive) = self.editor.selected_archive_mut() {
                        archive.set_rename(entry_index);
                        if let Some(entry) = archive.entries.get(entry_index) {
                            self.rename_buffer = entry.file_name.to_string();
                        }
                    }
                    self.rename_focused = true;
                    Task::batch(vec![
                        self.refresh_inspection(),
                        iced::widget::operation::focus(iced::widget::Id::new(RENAME_INPUT_ID)),
                    ])
                } else {
                    Task::none()
                };
                Task::batch(vec![task, Task::none()])
            }
            Message::EntryRightClicked(display_row) => {
                let task = if let Some(entry_index) = self.display_row_to_entry(display_row) {
                    self.editor.select_context_entry(entry_index);
                    self.clear_stale_viewer_load();
                    self.reset_texture_preview_state();
                    self.context_menu = Some((entry_index, display_row));
                    self.refresh_inspection()
                } else {
                    Task::none()
                };
                Task::batch(vec![task, Task::none()])
            }
            Message::EntryContextAction(action) => {
                self.context_menu = None;
                match action {
                    EntryAction::CopyName => {
                        if let Some(archive_index) = self.editor.selected_archive()
                            && let Some(entry_index) = self.editor.selected_entry()
                            && let Some(archive) = self.editor.archives().get(archive_index)
                            && let Some(entry) = archive.entries.get(entry_index)
                        {
                            let name = entry.file_name.to_string();
                            self.toast = Some(format!("Copied name: {}", name));
                            return iced::clipboard::write::<Message>(name);
                        }
                        Task::none()
                    }
                    EntryAction::Rename => Task::done(Message::StartRename),
                    EntryAction::Delete => {
                        self.editor.delete_selected();
                        Task::batch(vec![self.refresh_inspection(), Task::none()])
                    }
                    EntryAction::Export => {
                        self.last_export_selected_only = true;
                        dialogs::save_folder().map(Message::ExportFolderResult)
                    }
                    EntryAction::ViewTextures => self.load_selected_texture(),
                    EntryAction::ExportEmbeddedTextures => {
                        let Some(archive_index) = self.editor.selected_archive() else {
                            return Task::none();
                        };
                        let Some(entry_index) = self.editor.selected_entry() else {
                            return Task::none();
                        };
                        let (nif_basename, archive_path) = {
                            let Some(archive) = self.editor.archives().get(archive_index) else {
                                return Task::none();
                            };
                            let Some(entry) = archive.entries.get(entry_index) else {
                                return Task::none();
                            };
                            let stem = std::path::Path::new(&entry.file_name)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_string());
                            let stem = match stem {
                                Some(s) => s,
                                None => {
                                    self.toast = Some(format!(
                                        "Cannot determine basename of {}",
                                        entry.file_name
                                    ));
                                    return Task::none();
                                }
                            };
                            (stem, archive.path.clone())
                        };
                        let _ = archive_path;
                        Task::done(Message::ExportEmbeddedTexturesRequest {
                            entry_index,
                            nif_basename,
                        })
                    }
                    EntryAction::Render => {
                        dev_logger::breadcrumb("user: open in 3D viewer (in-app)");
                        self.load_selected_nif(InspectorTab::Model3D)
                    }
                    EntryAction::RenderExternal => {
                        dev_logger::breadcrumb("user: open in external viewer (PLY)");
                        let Some(archive_index) = self.editor.selected_archive() else {
                            return Task::none();
                        };
                        let Some(entry_index) = self.editor.selected_entry() else {
                            return Task::none();
                        };
                        let (entry_clone, archive_path, name) = {
                            let Some(archive) = self.editor.archives().get(archive_index) else {
                                return Task::none();
                            };
                            let Some(entry) = archive.entries.get(entry_index) else {
                                return Task::none();
                            };
                            (
                                entry.clone(),
                                archive.path.clone(),
                                entry.file_name.to_string(),
                            )
                        };
                        let data = match crate::parser::read_entry_data_from_source(
                            &entry_clone,
                            archive_path.as_deref(),
                        ) {
                            Ok(d) => d,
                            Err(e) => {
                                self.toast = Some(format!("Failed to read {name}: {e}"));
                                return Task::none();
                            }
                        };

                        if name.to_lowercase().ends_with(".dff") {
                            let rx = viewer3d::spawn_dff_render_window(data, name.clone());
                            self.viewer_rxs.push(rx);
                        } else if name.to_lowercase().ends_with(".col") {
                            let rx = viewer3d::spawn_col_render_window(data, name.clone());
                            self.viewer_rxs.push(rx);
                        } else {
                            let game_root = archive_path
                                .as_ref()
                                .and_then(|p| p.parent().and_then(|stream| stream.parent()))
                                .map(|p| p.to_path_buf());
                            let rx = viewer3d::spawn_render_window(data, name.clone(), game_root);
                            self.viewer_rxs.push(rx);
                        }

                        if let Some(archive) = self.editor.selected_archive_mut() {
                            archive.add_log(format!("Opening external 3D viewer for {name}"));
                        }
                        Task::none()
                    }
                }
            }

            Message::ShowAbout => {
                self.show_about = true;
                Task::none()
            }
            Message::HideAbout => {
                self.show_about = false;
                Task::none()
            }
            Message::ShowWelcome => {
                self.show_welcome = true;
                Task::none()
            }
            Message::HideWelcome => {
                self.show_welcome = false;
                if self.welcome_persist {
                    self.config.first_run_complete = true;
                }
                self.save_config();
                Task::none()
            }
            Message::ToggleWelcomePersist(val) => {
                self.welcome_persist = val;
                Task::none()
            }
            Message::ToggleUpdateDisabled(val) => {
                self.config.update_check_enabled = !val;
                self.save_config();
                Task::none()
            }
            Message::ToggleUpdateNotifyDisabled(val) => {
                self.config.update_notify_disabled = val;
                self.save_config();
                Task::none()
            }
            Message::ShowUnsupported(path) => {
                self.show_unsupported = Some(path);
                Task::none()
            }
            Message::HideUnsupported => {
                self.show_unsupported = None;
                Task::none()
            }
            Message::VisitRepository => {
                App::visit_repository();
                Task::none()
            }
            Message::HideUpdateStatus => {
                self.show_update_status = None;
                Task::none()
            }

            Message::CheckUpdatesManual => {
                self.update_check_manual = true;
                self.update_state = UpdateState::Checking;
                let repo = UPDATER_REPO.to_string();
                let current = env!("CARGO_PKG_VERSION").to_string();
                Task::perform(
                    check_updates_future(repo, current),
                    Message::UpdateResultReceived,
                )
            }
            Message::UpdateResultReceived(result) => {
                let was_manual = self.update_check_manual;
                self.update_check_manual = false;
                let suppressed = !was_manual && self.config.update_notify_disabled;
                match result {
                    UpdateResult::Available { version, url } => {
                        self.update_state = UpdateState::Available {
                            version: version.clone(),
                            url,
                        };
                        if !suppressed {
                            self.show_update_status = Some(format!("Update available: {version}"));
                        }
                    }
                    UpdateResult::UpToDate => {
                        self.update_state = UpdateState::UpToDate;
                        if !suppressed {
                            self.show_update_status =
                                Some("You are using the latest version.".into());
                        }
                    }
                    UpdateResult::Error(err) => {
                        self.update_state = UpdateState::Error(err.clone());
                        if !suppressed {
                            self.show_update_status = Some(format!("Update check failed: {err}"));
                        }
                    }
                }
                Task::none()
            }

            Message::SetTheme(theme) => {
                self.config.theme = theme;
                self.save_config();
                Task::none()
            }
            Message::ToastTimeout => {
                self.toast = None;
                Task::none()
            }
            Message::TickProgress => {
                self.poll_viewer_rxs();
                // Animate the progress bar smoothly towards the current value.
                // The baseline must fall back to the current value when no
                // animation is registered (get() reads 0.0 and finished
                // animations are reaped): comparing against 0 re-armed a
                // 0 -> current fill every tick, glitching the bar.
                if let Some(archive_idx) = self.editor.selected_archive() {
                    let current_progress =
                        self.editor.archives()[archive_idx].progress.percentage();
                    let visual = self
                        .animator
                        .get_or(ANIM_PROGRESS, current_progress);
                    if (visual - current_progress).abs() > 0.005 {
                        self.animator.animate_from_current(
                            ANIM_PROGRESS,
                            current_progress,
                            Duration::from_millis(200),
                            crate::ui::easing::Easing::CubicOut,
                        );
                    }
                }
                // Toast pulse: a finite random number (3–6) of smooth
                // green → neutral → green cycles, then settle.
                let toast_active = self.toast.is_some();
                if toast_active && self.toast_pulses_remaining == 0 {
                    // Toast just appeared: start N random pulses (3–6).
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    let count = 3 + (seed % 4) as u32;
                    self.toast_pulses_remaining = count;
                    self.toast_pulse_target = 1.0;
                    self.animator.animate_from_current(
                        ANIM_TOAST_OPACITY,
                        self.toast_pulse_target,
                        Duration::from_millis(300),
                        crate::ui::easing::Easing::CubicOut,
                    );
                } else if toast_active && self.toast_pulses_remaining > 0 {
                    // A pulse finished: chain to the next if more remain.
                    if !self.animator.is_running(ANIM_TOAST_OPACITY) {
                        self.toast_pulses_remaining -= 1;
                        if self.toast_pulses_remaining > 0 {
                            // Toggle target: 1.0 → 0.0 → 1.0 → 0.0 → ...
                            self.toast_pulse_target = if self.toast_pulse_target > 0.5 {
                                0.0
                            } else {
                                1.0
                            };
                            self.animator.animate(
                                ANIM_TOAST_OPACITY,
                                self.animator.get(ANIM_TOAST_OPACITY),
                                self.toast_pulse_target,
                                Duration::from_millis(300),
                                crate::ui::easing::Easing::CubicOut,
                            );
                        } else {
                            // Final pulse: settle on green.
                            self.toast_pulse_target = 1.0;
                            self.animator.animate(
                                ANIM_TOAST_OPACITY,
                                self.animator.get(ANIM_TOAST_OPACITY),
                                1.0,
                                Duration::from_millis(300),
                                crate::ui::easing::Easing::CubicOut,
                            );
                        }
                    }
                } else if !toast_active && self.toast_pulses_remaining > 0 {
                    // Toast was cleared mid-pulse: fade back immediately.
                    self.toast_pulses_remaining = 0;
                    self.toast_pulse_target = 0.0;
                    self.animator.animate_from_current(
                        ANIM_TOAST_OPACITY,
                        0.0,
                        Duration::from_millis(200),
                        crate::ui::easing::Easing::CubicOut,
                    );
                }
                // Reap finished animations to keep the animator lean.
                self.animator.reap_finished();
                self.prepare_interaction_animation();
                Task::none()
            }
            Message::AnimationTick(now) => {
                let mut autoscroll_task = Task::none();
                if let Some(prev) = self.prev_tick {
                    // A window can be suspended or the subscription can be
                    // restarted after a long idle period. Cap one frame so a
                    // short interaction remains visible instead of completing
                    // instantly after resume.
                    let dt = now
                        .saturating_duration_since(prev)
                        .min(Duration::from_millis(50));
                    self.animator.update(dt);
                    if self.viewer_load.is_some() {
                        self.viewer_load_phase =
                            (self.viewer_load_phase + dt.as_secs_f32() * 0.72).fract();
                    }
                    if self.has_active_progress() {
                        self.shimmer_phase =
                            (self.shimmer_phase + dt.as_secs_f32() * 0.9).fract();
                    }
                    if self.editor.archives().is_empty() && self.config.motion_enabled {
                        // Slow idle clock for the empty-state hero (~6.7 s
                        // drift cycle).
                        self.empty_state_phase =
                            (self.empty_state_phase + dt.as_secs_f32() * 0.15).fract();
                    }
                    autoscroll_task = self.advance_autoscroll_momentum(dt);
                }
                self.prev_tick = Some(now);

                self.clear_stale_viewer_load();

                if !self.animator.is_running(ANIM_ENTRY_FEEDBACK) {
                    self.entry_feedback_target = None;
                }
                if !self.animator.is_running(ANIM_ARCHIVE_TAB_FEEDBACK) {
                    self.archive_tab_feedback_target = None;
                }
                if !self.animator.is_running(ANIM_INSPECTOR_TAB_FEEDBACK) {
                    self.inspector_tab_feedback_target = None;
                }
                if !self.animator.is_running(ANIM_CLICK_RIPPLE) {
                    self.ripple = None;
                }

                // Auto-dismiss toasts after their duration elapses so the
                // green status pulse does not appear to stay on
                // indefinitely. The default is snappy; notices that need
                // read time opt into a longer duration.
                if self.toast.is_some() {
                    match self.toast_start {
                        None => self.toast_start = Some(now),
                        Some(start) => {
                            if now.duration_since(start) >= self.toast_dismiss_after {
                                self.toast = None;
                                self.toast_start = None;
                                self.toast_dismiss_after = Duration::from_millis(2500);
                                self.toast_extended_duration = false;
                            }
                        }
                    }
                } else {
                    self.toast_start = None;
                }

                // Floating toast snackbar reveal: slide+fade in when the
                // toast text changes, fade out once the toast clears.
                if let Some(current) = self.toast.clone() {
                    let is_new = self.toast_reveal_text.as_ref() != Some(&current);
                    if is_new {
                        self.toast_reveal_text = Some(current);
                        // A fresh toast resets to the snappy dismissal
                        // unless the setter opted into a long notice.
                        self.toast_dismiss_after = if self.toast_extended_duration {
                            Duration::from_millis(6500)
                        } else {
                            Duration::from_millis(2500)
                        };
                        self.toast_extended_duration = false;
                    }
                    if is_new || self.toast_reveal_fading {
                        self.toast_reveal_fading = false;
                        if self.config.motion_enabled {
                            self.animator.animate(
                                ANIM_TOAST_REVEAL,
                                self.animator.get_or(ANIM_TOAST_REVEAL, 0.0),
                                1.0,
                                Duration::from_millis(220),
                                crate::ui::easing::Easing::CubicOut,
                            );
                        }
                    }
                } else if self.toast_reveal_text.is_some() {
                    if !self.config.motion_enabled {
                        self.toast_reveal_text = None;
                        self.toast_reveal_fading = false;
                        self.animator.cancel(ANIM_TOAST_REVEAL);
                    } else if !self.toast_reveal_fading {
                        self.toast_reveal_fading = true;
                        self.animator.animate(
                            ANIM_TOAST_REVEAL,
                            self.animator.get_or(ANIM_TOAST_REVEAL, 1.0),
                            0.0,
                            Duration::from_millis(220),
                            crate::ui::easing::Easing::CubicOut,
                        );
                    } else if !self.animator.is_running(ANIM_TOAST_REVEAL) {
                        self.toast_reveal_text = None;
                        self.toast_reveal_fading = false;
                    }
                }

                self.prepare_interaction_animation();

                autoscroll_task
            }
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                Task::none()
            }
            Message::ScrollOffsetChanged { y, max_y } => {
                self.entry_table_max_scroll_y = max_y.max(0.0);
                self.entry_table_viewport_known = true;
                self.scroll_y = y.clamp(0.0, self.entry_table_max_scroll_y);
                Task::none()
            }
            Message::HideContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::ModifiersChanged(mods) => {
                self.modifiers = mods;
                Task::none()
            }
            Message::PointerMoved(position) => {
                self.last_pointer_position = Some(position);
                self.update_autoscroll_momentum(position);
                Task::none()
            }
            Message::EntryTableHoverChanged(hovered) => {
                self.entry_table_hovered = hovered;
                Task::none()
            }
            Message::AutoScrollStarted => {
                if self.autoscroll {
                    self.end_autoscroll();
                    return Task::none();
                }

                // Middle-clicking while the context menu is open only dismisses it.
                if self.context_menu.take().is_some() {
                    return Task::none();
                }

                let has_entries = self
                    .editor
                    .selected_archive()
                    .and_then(|index| self.editor.archives().get(index))
                    .is_some_and(|archive| !archive.selected_indices.is_empty());
                if !self.entry_table_hovered || !has_entries {
                    return Task::none();
                }

                self.autoscroll = true;
                self.autoscroll_momentum.begin(self.last_pointer_position);
                if !self.autoscroll_notice_shown {
                    self.autoscroll_notice_shown = true;
                    self.toast = Some(
                        "Autoscroll active: move the pointer to scroll. Click, middle-click, right-click, use the wheel, or press a key to stop."
                            .to_string(),
                    );
                    self.toast_extended_duration = true;
                }
                Task::none()
            }
            Message::AutoScrollEnded => {
                self.end_autoscroll();
                Task::none()
            }
            Message::AutoScrollEscape => {
                if self.predictions_open() {
                    self.predictions_dismissed = true;
                    self.prediction_index = None;
                }
                self.end_autoscroll();
                Task::none()
            }
            Message::OpenLastExportFolder => {
                if let Some(index) = self.editor.selected_archive()
                    && let Some(archive) = self.editor.archives().get(index)
                    && let Some(folder) = archive.last_export_folder.clone()
                {
                    open_export_folder(&folder);
                }
                Task::none()
            }
            Message::SortBy(column) => {
                let updated_chain = if let Some(archive) = self.editor.selected_archive_mut() {
                    let unique_types =
                        archive.unique_file_types(self.config.literal_file_types).to_vec();
                    match column {
                        SortColumn::Name => {
                            if archive.sort.column == SortColumn::Name {
                                archive.sort.direction = match archive.sort.direction {
                                    SortDirection::Ascending => SortDirection::Descending,
                                    SortDirection::Descending => SortDirection::Ascending,
                                };
                            } else {
                                archive.sort.column = SortColumn::Name;
                                archive.sort.direction = SortDirection::Ascending;
                            }
                        }
                        SortColumn::Type => {
                            if archive.sort.column == SortColumn::Type {
                                let count = unique_types.len().max(1);
                                archive.sort.type_index = (archive.sort.type_index + 1) % count;
                            } else {
                                archive.sort.column = SortColumn::Type;
                                archive.sort.type_index = 0;
                                archive.sort.direction = SortDirection::Ascending;
                            }
                        }
                        SortColumn::Size => {
                            if archive.sort.column == SortColumn::Size {
                                archive.sort.direction = match archive.sort.direction {
                                    SortDirection::Ascending => SortDirection::Descending,
                                    SortDirection::Descending => SortDirection::Ascending,
                                };
                            } else {
                                archive.sort.column = SortColumn::Size;
                                archive.sort.direction = SortDirection::Descending;
                            }
                        }
                    }

                    archive.sort_chain = match archive.sort.column {
                        SortColumn::Name => SortChain::new(vec![SortPriority::new(
                            SortKey::Name,
                            archive.sort.direction,
                        )]),
                        SortColumn::Type => SortChain::new(vec![
                            SortPriority::new(SortKey::Type, SortDirection::Ascending),
                            SortPriority::new(SortKey::Name, SortDirection::Ascending),
                        ]),
                        SortColumn::Size => SortChain::new(vec![SortPriority::new(
                            SortKey::Size,
                            archive.sort.direction,
                        )]),
                    };
                    archive.sync_sort_state_from_chain();
                    let filter = self.search.clone();
                    archive.update_selected_list(&filter, self.config.literal_file_types);
                    Some(archive.sort_chain.clone())
                } else {
                    None
                };

                if let Some(updated_chain) = updated_chain {
                    // Promote the current chain to the global default
                    // so the next archive opened inherits this sort.
                    // Cheap, since the chain is at most 10 priorities.
                    self.config.default_sort_chain = updated_chain.clone();
                    self.editor.set_default_sort_chain(updated_chain);
                    self.save_config();
                }
                Task::none()
            }

            Message::OpenValidatorPopup => self.open_validator_popup(),
            Message::TargetProbed { archive_index, hint } => {
                if let Some(archive) = self.editor.archives_mut().get_mut(archive_index) {
                    archive.target_hint = hint;
                }
                Task::none()
            }
            Message::CloseValidatorPopup => {
                self.validator_popup_open = false;
                Task::none()
            }
            Message::SetCompatHighlight(enabled) => {
                self.compat_highlight_enabled = enabled;
                Task::none()
            }
            Message::TabResizeStarted => {
                self.tab_resize_drag = Some(f32::NAN);
                Task::none()
            }
            Message::TabResizeMoved(x) => {
                if let Some(last) = self.tab_resize_drag
                    && last.is_finite()
                {
                    self.archive_tab_width = crate::config::clamp_archive_tab_width(
                        self.archive_tab_width + (x - last),
                    );
                }
                self.tab_resize_drag = Some(x);
                Task::none()
            }
            Message::TabResizeEnded => {
                if self.tab_resize_drag.take().is_some() {
                    self.config.archive_tab_width = self.archive_tab_width;
                    self.save_config();
                }
                Task::none()
            }
            Message::ValidateArchiveFor(target_id) => {
                let Some(archive_index) = self.editor.selected_archive() else {
                    self.toast = Some("Open an archive first to validate it.".into());
                    return Task::none();
                };
                if self.editor.archives()[archive_index].progress.in_use() {
                    self.toast = Some("Another task is still running.".into());
                    return Task::none();
                }
                // Picking a game both selects and persists the archive's
                // target, then runs the validation against it.
                let mut archive_path = None;
                if let Some(archive) = self.editor.archives_mut().get_mut(archive_index) {
                    archive.target_game = Some(target_id);
                    archive_path = archive.path.clone();
                }
                if let Some(path) = archive_path {
                    self.config.set_archive_target(&path, target_id);
                    self.save_config();
                }
                // The popup is a picker: running closes it so the user
                // sees the highlighted rows underneath.
                self.validator_popup_open = false;
                // The snapshot shares the archive's ProgressInfo (Arc), so
                // the toolbar progress bar and its cancel button drive the
                // background scan.
                let snapshot = self.editor.archives()[archive_index].clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::compat::scan::validate_open_archive(
                                &snapshot,
                                &crate::compat::scan::ScanOptions {
                                    decode_pixels: false,
                                    target: Some(target_id),
                                },
                            )
                        })
                        .await
                        .unwrap_or_else(|err| Err(anyhow::anyhow!("task panicked: {err}")))
                    },
                    move |result| Message::CompatibilityValidated {
                        archive_index,
                        result: result.map_err(|err| format!("{err}")),
                    },
                )
            }
            Message::CompatibilityValidated { archive_index, result } => {
                match result {
                    Ok(report) => {
                        let Some(archive) = self.editor.archives_mut().get_mut(archive_index) else {
                            self.toast = Some("The validated archive was closed.".into());
                            return Task::none();
                        };
                        let errors = report.error_count();
                        let warnings = report.warning_count();
                        let target = report.target.unwrap_or("gta3");
                        let display = crate::compat::games::profile_by_id(target)
                            .map(|game| game.display)
                            .unwrap_or(target);
                        let verdict_summary = report
                            .verdicts
                            .get(target)
                            .map(|counts| {
                                counts
                                    .iter()
                                    .map(|(verdict, count)| format!("{verdict} {count}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_else(|| "no textures".to_string());
                        let summary = format!(
                            "Validated {} TXDs ({} textures) for {display}: {verdict_summary}; {errors} errors, {warnings} warnings",
                            report.txd_entries, report.textures
                        );
                        archive.add_log(format!("Compatibility check: {summary}"));
                        for (code, count) in &report.anomaly_counts {
                            archive.add_log(format!(
                                "  {code}: {count} (e.g. {})",
                                report
                                    .anomaly_examples
                                    .get(code)
                                    .and_then(|examples| examples.first())
                                    .cloned()
                                    .unwrap_or_default()
                            ));
                        }
                        archive.compat_report = Some(report);
                        // The summary is long; keep it readable (6.5 s
                        // instead of the snappy default).
                        self.toast_extended_duration = true;
                        self.toast = Some(if errors == 0 && warnings == 0 {
                            format!("No compatibility issues found - {summary}")
                        } else {
                            summary
                        });

                        // A save may be waiting on this report: review it
                        // or continue silently.
                        if let Some(pending) = self.pending_save.take() {
                            if pending.index != archive_index || pending.issue.is_some() {
                                self.pending_save = Some(pending);
                            } else {
                                let issue = self
                                    .editor
                                    .archives()
                                    .get(archive_index)
                                    .and_then(|archive| {
                                        archive.compat_report.as_ref().map(|report| {
                                            crate::compat::save::evaluate_save(report, archive)
                                        })
                                    });
                                match issue {
                                    Some(issue) if issue.needs_review() => {
                                        let fix = issue.has_fixable();
                                        self.pending_save = Some(PendingSave {
                                            issue: Some(issue),
                                            fix,
                                            ..pending
                                        });
                                        self.toast = None;
                                        return Task::none();
                                    }
                                    _ => {
                                        let archive =
                                            self.editor.archives()[archive_index].clone();
                                        return self.run_save(
                                            archive,
                                            pending.path,
                                            pending.version,
                                            pending.remove_existing,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(err) if err.contains("cancelled") => {
                        self.toast = Some("Validation cancelled.".into());
                    }
                    Err(err) => {
                        self.toast_extended_duration = true;
                        self.toast = Some(format!("Validation failed: {err}"));
                    }
                }
                Task::none()
            }

            Message::FilesDropped(path) => {
                if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("img"))
                {
                    return self.open_archive_path(path);
                }
                let Some((index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast =
                        Some("Open an archive first to drop non-IMG files into it.".into());
                    return Task::none();
                };
                self.begin_import(index, archive, vec![path], None)
            }

            Message::TextureDecodeRequested => {
                let Some(entry_index) = self.editor.selected_entry() else {
                    return Task::none();
                };
                // Cache miss or first request: decode in the background.
                self.selected_texture = 0;
                self.show_texture_uv = false;
                self.decode_texture_entry(entry_index)
            }

            Message::TextureDecoded {
                archive_index,
                index,
                result,
            } => {
                let is_active = self.selected_entry_key() == Some((archive_index, index));
                match result {
                    Ok(textures) => {
                        if let Some(archive) = self.editor.archives_mut().get_mut(archive_index) {
                            // The decode task already published via its
                            // placeholder guard; re-insert only if the entry
                            // was evicted between publish and completion.
                            if !archive.texture_cache.contains_key(&index) {
                                archive.texture_cache.insert(index, Arc::clone(&textures));
                            }
                            let count = textures.len();
                            archive.add_log(format!("Decoded {count} texture preview(s)"));
                            if is_active {
                                self.toast = Some(format!("Decoded {count} texture(s)"));
                            }
                        }
                    }
                    Err(err) => {
                        if is_active {
                            self.toast = Some(err);
                        }
                    }
                }
                Task::none()
            }

            Message::TextureSelect(index) => {
                self.selected_texture = index;
                Task::none()
            }

            Message::TextureExport => {
                dialogs::save_folder().map(Message::TextureExportFolderResult)
            }

            Message::TextureExportFolderResult(Some(folder)) => {
                let Some(archive_index) = self.editor.selected_archive() else {
                    return Task::none();
                };
                let Some(entry_index) = self.editor.selected_entry() else {
                    return Task::none();
                };
                let textures = self
                    .editor
                    .archives()
                    .get(archive_index)
                    .and_then(|a| a.texture_cache.get(&entry_index));
                let Some(textures) = textures else {
                    self.toast = Some("No decoded textures to export.".into());
                    return Task::none();
                };

                let folder = folder.clone();
                let count = textures.len();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || -> Result<(), String> {
                            for tex in textures.iter() {
                                let safe_name: String = tex
                                    .name
                                    .chars()
                                    .map(|c| {
                                        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                                            c
                                        } else {
                                            '_'
                                        }
                                    })
                                    .collect();
                                let path = folder.join(format!("{}.tga", safe_name));
                                let mut tga = Vec::with_capacity(18 + tex.rgba.len());
                                tga.push(0);
                                tga.push(0);
                                tga.push(2);
                                tga.extend_from_slice(&[0, 0, 0, 0, 0]);
                                tga.extend_from_slice(&[0, 0]);
                                tga.extend_from_slice(&[0, 0]);
                                tga.extend_from_slice(&(tex.width as u16).to_le_bytes());
                                tga.extend_from_slice(&(tex.height as u16).to_le_bytes());
                                tga.push(32);
                                tga.push(0x20);
                                for chunk in tex.rgba.chunks_exact(4) {
                                    tga.push(chunk[2]);
                                    tga.push(chunk[1]);
                                    tga.push(chunk[0]);
                                    tga.push(chunk[3]);
                                }
                                std::fs::write(&path, tga).map_err(|e| {
                                    format!("Failed to write {}: {e}", path.display())
                                })?;
                            }
                            Ok(())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("task panicked: {e}")))
                    },
                    move |result| {
                        if result.is_ok() {
                            let _ = format!("Exported {count} texture(s)");
                        }
                        Message::Noop
                    },
                )
            }

            Message::TextureExportFolderResult(None) => Task::none(),

            Message::TextureUvToggled(show) => {
                self.show_texture_uv = show;
                Task::none()
            }

            Message::ViewTextureGridToggled(show) => {
                self.show_texture_grid = show;
                self.config.show_texture_grid = show;
                self.save_config();
                Task::none()
            }

            Message::ViewTextureGridSize(divisions) => {
                self.texture_grid_divisions = divisions;
                self.config.texture_grid_divisions = divisions;
                self.save_config();
                Task::none()
            }

            Message::SetNavigationGizmoVisible(show) => {
                self.config.show_navigation_gizmo = show;
                self.viewer3d_handle.set_navigation_visible(show);
                self.save_config();
                Task::none()
            }

            Message::ToggleAutoscrollMomentum(enabled) => {
                self.config.autoscroll_momentum_enabled = enabled;
                if !enabled {
                    self.autoscroll_momentum.clear_tail();
                    self.prepare_interaction_animation();
                }
                self.save_config();
                Task::none()
            }
            Message::ToggleMotionEffects(enabled) => {
                self.config.motion_enabled = enabled;
                if !enabled {
                    self.stop_interaction_animations();
                }
                self.save_config();
                Task::none()
            }
            Message::ToggleSelectionPulse(enabled) => {
                self.config.selection_pulse_enabled = enabled;
                if !enabled {
                    self.animator.cancel(ANIM_ENTRY_FEEDBACK);
                    self.animator.cancel(ANIM_ARCHIVE_TAB_FEEDBACK);
                    self.animator.cancel(ANIM_INSPECTOR_TAB_FEEDBACK);
                    self.entry_feedback_target = None;
                    self.archive_tab_feedback_target = None;
                    self.inspector_tab_feedback_target = None;
                }
                self.save_config();
                Task::none()
            }
            Message::ToggleClickRipple(enabled) => {
                self.config.click_ripple_enabled = enabled;
                if !enabled {
                    self.animator.cancel(ANIM_CLICK_RIPPLE);
                    self.ripple = None;
                }
                self.save_config();
                Task::none()
            }
            Message::ToggleIconMicroMotion(enabled) => {
                self.config.icon_micro_motion_enabled = enabled;
                self.save_config();
                Task::none()
            }
            Message::ToggleSearchBar(show) => {
                self.config.show_search_bar = show;
                if !show {
                    // The filter box is gone, so a stale filter would
                    // silently hide entries. Reset to show everything.
                    self.search.clear();
                    self.filter_pending = true;
                    self.search_focused = false;
                    self.close_predictions();
                }
                self.save_config();
                Task::none()
            }

            Message::ToggleLiteralFileTypes(literal) => {
                self.config.literal_file_types = literal;
                self.editor.file_type_literal = literal;
                for archive in self.editor.archives_mut() {
                    archive.invalidate_type_cache();
                }
                self.filter_pending = true;
                self.save_config();
                Task::none()
            }

            Message::ToggleContextAccumulate(accumulates) => {
                self.config.context_selection_accumulates = accumulates;
                self.editor.context_selection_accumulates = accumulates;
                self.save_config();
                Task::none()
            }

            Message::ExportEmbeddedTexturesRequest {
                entry_index,
                nif_basename,
            } => {
                let _ = entry_index;
                self.toast = Some(format!(
                    "Pick a folder to export embedded textures from {nif_basename}"
                ));
                let nb = nif_basename.clone();
                dialogs::save_folder().map(move |folder| {
                    Message::ExportEmbeddedTexturesFolderResult {
                        entry_index,
                        nif_basename: nb.clone(),
                        folder,
                    }
                })
            }
            Message::ExportEmbeddedTexturesFolderResult {
                entry_index,
                nif_basename,
                folder: Some(folder),
            } => {
                let archive_path = self
                    .editor
                    .selected_archive()
                    .and_then(|i| self.editor.archives().get(i))
                    .and_then(|a| a.path.clone());
                let game_root = archive_path
                    .as_deref()
                    .and_then(|p| p.parent().and_then(|stream| stream.parent()))
                    .map(|p| p.to_path_buf());
                let Some(game_root) = game_root else {
                    self.toast =
                        Some("Could not determine game root from archive path".to_string());
                    return Task::none();
                };
                let nb_for_callback = nif_basename.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let ide_map = crate::inspector::texture::IdeMap::build(&game_root);
                            crate::inspector::texture_export::export_embedded_textures(
                                &nif_basename,
                                &ide_map,
                                &folder,
                            )
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("export task panicked: {e}")))
                    },
                    move |result| Message::ExportEmbeddedTexturesCompleted {
                        entry_index,
                        nif_basename: nb_for_callback.clone(),
                        result,
                    },
                )
            }
            Message::ExportEmbeddedTexturesFolderResult { folder: None, .. } => Task::none(),
            Message::ExportEmbeddedTexturesCompleted {
                entry_index,
                nif_basename,
                result,
            } => {
                let _ = entry_index;
                let now = chrono::Local::now().format("%H:%M:%S");
                let archive_index = self.editor.selected_archive().unwrap_or(0);
                if let Some(archive) = self.editor.archives_mut().get_mut(archive_index) {
                    match &result {
                        Ok(report) => {
                            let line = format!("[{}] {}: {}", now, nif_basename, report.summary());
                            archive.recent_exports.push(line.clone());
                            archive.add_log(line);
                            self.toast = Some(report.summary());
                        }
                        Err(err) => {
                            let line = format!("[{}] {} export failed: {err}", now, nif_basename);
                            archive.add_log(line.clone());
                            self.toast = Some(line);
                        }
                    }
                }
                Task::none()
            }
            Message::Viewer3dLoadSelected => {
                let target_tab = self.selected_inspector_tab;
                self.load_selected_nif(target_tab)
            }
            Message::Viewer3dRequestLoad {
                archive_index,
                entry_index,
            } => {
                let (entry_clone, archive_path, archive_entries, cache_key) = {
                    let Some(archive) = self.editor.archives().get(archive_index) else {
                        return Task::none();
                    };
                    let Some(entry) = archive.entries.get(entry_index) else {
                        return Task::none();
                    };
                    (
                        entry.clone(),
                        archive.path.clone(),
                        archive.entries.clone(),
                        (archive.file_name.clone(), archive.generation(), entry_index),
                    )
                };
                let nif_basename = std::path::Path::new(&entry_clone.file_name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| entry_clone.file_name.to_string());
                let is_dff = entry_clone.file_name.to_ascii_lowercase().ends_with(".dff");
                // Reuse a memoized IdeMap for this game root when one has
                // already been built; otherwise the background task builds
                // one and hands it back for memoization.
                let ide_map_hit: Option<BuiltIdeMap> = {
                    let game_root = archive_path
                        .as_deref()
                        .and_then(|p| p.parent().and_then(|stream| stream.parent()))
                        .map(|p| p.to_path_buf());
                    match game_root {
                        Some(root) => self.ide_maps.get(&root).map(|map| (root, Arc::clone(map))),
                        None => None,
                    }
                };
                let scene_cache = Arc::clone(&self.scene_cache);
                Task::perform(
                    async move {
                        // Single-flight: the scene cache hands the
                        // placeholder guard to exactly one loader per
                        // (archive, generation, entry). A zero-timeout turn
                        // into `Timeout` while another load runs and into
                        // `Value` if the scene landed meanwhile - both skip
                        // the work. Holding the guard across the build also
                        // deduplicates the fallback `IdeMap` scan below.
                        match scene_cache.get_value_or_guard(&cache_key, Some(Duration::ZERO)) {
                            GuardResult::Value(_) | GuardResult::Timeout => Message::Noop,
                            GuardResult::Guard(guard) => {
                                let (result, ide_map_new) =
                                    tokio::task::spawn_blocking(move || {
                                        let (ide_map, ide_map_new): (
                                            Option<Arc<crate::inspector::texture::IdeMap>>,
                                            Option<BuiltIdeMap>,
                                        ) = match ide_map_hit {
                                            Some((_, map)) => (Some(map), None),
                                            None => match archive_path
                                    .as_deref()
                                    .and_then(|p| p.parent().and_then(|stream| stream.parent()))
                                    .map(|p| p.to_path_buf())
                                {
                                    Some(root) => {
                                        let map = Arc::new(
                                            crate::inspector::texture::IdeMap::build(&root),
                                        );
                                        (Some(Arc::clone(&map)), Some((root, map)))
                                    }
                                    None => (None, None),
                                },
                            };
                            let result = (|| -> Result<crate::inspector::scene3d::Scene, String> {
                                let bytes = crate::parser::read_entry_data_from_source(
                                    &entry_clone,
                                    archive_path.as_deref(),
                                )
                                .map_err(|e| format!("I/O: {e}"))?;
                                let archive_texture_index =
                                    crate::inspector::texture::ArchiveTextureIndex::from_entries(
                                        &archive_entries,
                                        archive_path.as_deref(),
                                    );
                                if is_dff {
                                    let dff_meshes = crate::parser::dff::parse_dff(&bytes)
                                        .map_err(|e| format!("DFF parse: {e}"))?;
                                    let texture_names = dff_meshes
                                        .iter()
                                        .filter_map(|mesh| mesh.texture_name.clone())
                                        .collect::<Vec<_>>();
                                    let renderware_textures =
                                        archive_texture_index.resolve_textures_for_dff(
                                            &nif_basename,
                                            &texture_names,
                                            ide_map.as_deref(),
                                        );
                                    let resolver = move |name: &str| {
                                        let key =
                                            crate::inspector::texture::texture_key(name);
                                        renderware_textures.get(&key).cloned()
                                    };
                                    let base =
                                        crate::inspector::scene3d::camera::BaseOrientation::Zup;
                                    return crate::inspector::scene3d::decode::build_scene_from_dff(
                                        &dff_meshes,
                                        base,
                                        resolver,
                                    )
                                        .map_err(|e| format!("scene: {e:?}"));
                                }
                                let nft_catalog = ide_map
                                    .as_deref()
                                    .and_then(|map| {
                                        crate::inspector::texture::resolve_textures_for_nif(
                                            &nif_basename,
                                            map,
                                        )
                                    })
                                    .or_else(|| {
                                        archive_texture_index.resolve_textures_for_nif(
                                            &nif_basename,
                                            ide_map.as_deref(),
                                        )
                                    });
                                let resolver = move |name: &str| {
                                    nft_catalog
                                        .as_ref()
                                        .and_then(|cat| cat.get_pixels(name))
                                        .and_then(SceneTexture::from_tga)
                                        .or_else(|| {
                                            archive_texture_index
                                                .read(name)
                                                .and_then(|bytes| SceneTexture::from_tga(&bytes))
                                        })
                                        .or_else(|| {
                                            ide_map
                                                .as_deref()
                                                .and_then(|map| map.locate_external_texture(name))
                                                .and_then(|path| std::fs::read(path).ok())
                                                .and_then(|bytes| {
                                                    SceneTexture::from_tga(&bytes)
                                                })
                                        })
                                };
                                let base =
                                    crate::inspector::scene3d::camera::BaseOrientation::Zup;
                                crate::inspector::scene3d::decode::parse_and_build_scene(
                                    &bytes, base, resolver,
                                )
                                .map_err(|e| format!("scene: {e:?}"))
                            })();
                             (result, ide_map_new)
                        })
                        .await
                        .unwrap_or_else(|e| {
                            (Err(format!("join: {e}")), None)
                        });
                                match result {
                                    Ok(scene) => {
                                        let scene = Arc::new(scene);
                                        // Publishing through the placeholder
                                        // keeps the single-flight atomic with
                                        // the insert. An `Err` means the cache
                                        // was invalidated mid-load (entries
                                        // changed), so the scene is stale.
                                        if guard.insert(Arc::clone(&scene)).is_err() {
                                            return Message::Noop;
                                        }
                                        Message::Viewer3dLoadCompleted {
                                            archive_index,
                                            entry_index,
                                            generation: cache_key.1,
                                            result: Ok(scene),
                                            ide_map: ide_map_new,
                                        }
                                    }
                                    Err(e) => {
                                        // Release the placeholder so a retry
                                        // can claim the load.
                                        drop(guard);
                                        Message::Viewer3dLoadCompleted {
                                            archive_index,
                                            entry_index,
                                            generation: cache_key.1,
                                            result: Err(e),
                                            ide_map: ide_map_new,
                                        }
                                    }
                                }
                            }
                        }
                    },
                    |message| message,
                )
            }
            Message::Viewer3dLoadCompleted {
                archive_index,
                entry_index,
                generation,
                result,
                ide_map,
            } => {
                if let Some((root, map)) = ide_map {
                    self.ide_maps.entry(root).or_insert(map);
                }
                let completed_target = (archive_index, entry_index);
                // The scene cache is generation-keyed but not cleared on
                // entry mutations, so an in-flight load completing after
                // entries changed must not resolve onto the new data at
                // the same index.
                let generation_matches = self
                    .editor
                    .archives()
                    .get(archive_index)
                    .is_some_and(|archive| archive.generation() == generation);
                if !generation_matches {
                    if self
                        .viewer_load
                        .as_ref()
                        .is_some_and(|load| load.target == completed_target)
                    {
                        self.clear_viewer_load();
                    }
                    return Task::none();
                }
                if self.editor.selected_archive() != Some(archive_index)
                    || self.editor.selected_entry() != Some(entry_index)
                {
                    if self
                        .viewer_load
                        .as_ref()
                        .is_some_and(|load| load.target == completed_target)
                    {
                        self.clear_viewer_load();
                    }
                    return Task::none();
                }
                self.clear_viewer_load();
                match result {
                    Ok(scene) => {
                        dev_logger::breadcrumb(&format!(
                            "3D load ok: {} verts, {} tris, {} textured meshes",
                            scene.total_vertices(),
                            scene.total_triangles(),
                            scene.textured_mesh_count()
                        ));
                        if let Some(archive) = self.editor.archives().get(archive_index) {
                            let key =
                                (archive.file_name.clone(), archive.generation(), entry_index);
                            // The load task published via its placeholder
                            // guard; re-insert only if the entry was evicted
                            // between publish and completion.
                            if !self.scene_cache.contains_key(&key) {
                                self.scene_cache.insert(key, Arc::clone(&scene));
                            }
                        }
                        self.store_scene_texture_previews(&scene, archive_index, entry_index);
                        self.viewer3d_handle.set_scene(scene);
                        self.active_viewer_entry = Some((archive_index, entry_index));
                        if let Some(archive) = self.editor.selected_archive_mut() {
                            archive.add_log("In-app 3D viewer ready".to_string());
                        }
                    }
                    Err(e) => {
                        dev_logger::breadcrumb(&format!("3D load failed: {e}"));
                        self.toast = Some(format!("3D load failed: {e}"));
                    }
                }
                Task::none()
            }
            Message::Viewer3dSelectTab(tab) => {
                self.start_inspector_tab_feedback(tab);
                self.start_click_ripple(RippleTarget::InspectorTab(tab));
                self.selected_inspector_tab = tab;
                self.refresh_active_preview()
            }
            Message::Viewer3dClear => {
                self.active_viewer_entry = None;
                self.clear_viewer_load();
                self.viewer3d_handle.clear();
                Task::none()
            }
            Message::Viewer3dReset => {
                self.viewer3d_handle.reset_camera();
                Task::none()
            }
            Message::Viewer3dToggleCenterOrigin => {
                self.viewer3d_handle.toggle_center_origin();
                Task::none()
            }
            Message::Viewer3dToggleWireframe => {
                self.viewer3d_handle.toggle_wireframe();
                Task::none()
            }
            Message::Viewer3dToggleGrid => {
                self.viewer3d_handle.toggle_grid();
                Task::none()
            }
            Message::Viewer3dToggleCullBackfaces => {
                self.viewer3d_handle.toggle_cull_back();
                Task::none()
            }
            Message::Viewer3dToggleTextured => {
                self.viewer3d_handle.toggle_textured();
                Task::none()
            }
            Message::Viewer3dToggleAlphaBlend => {
                self.viewer3d_handle.toggle_alpha_blend();
                Task::none()
            }

            // ---- Sort Manager dialog ----
            Message::OpenSortManager => {
                // Seed the draft from the active archive's chain. If
                // no archive is open, seed from the global default.
                // The `take` would be tempting but the dialog is the
                // only place the draft lives, so we just overwrite.
                let draft = self
                    .editor
                    .selected_archive()
                    .and_then(|i| self.editor.archives().get(i))
                    .map(|a| a.sort_chain.clone())
                    .unwrap_or_else(|| self.config.default_sort_chain.clone());
                self.sort_draft = Some(draft);
                self.show_sort_manager = true;
                Task::none()
            }
            Message::CloseSortManager => {
                // Discard the draft on close. "Apply" was the only
                // path that committed changes; everything else just
                // leaves the live archive alone.
                self.sort_draft = None;
                self.show_sort_manager = false;
                Task::none()
            }
            Message::SortResetDraft => {
                // Re-seed the draft from the active archive so the
                // user can undo in-progress edits without closing the
                // dialog.
                if let Some(draft) = self.sort_draft.as_mut()
                    && let Some(i) = self.editor.selected_archive()
                    && let Some(a) = self.editor.archives().get(i)
                {
                    *draft = a.sort_chain.clone();
                }
                Task::none()
            }
            Message::SortApplyDraft => {
                // Commit the draft to the live archive, the editor's
                // default chain (so new archives inherit), and the
                // persisted config (so the change survives a restart).
                if let Some(draft) = self.sort_draft.take() {
                    if let Some(i) = self.editor.selected_archive()
                        && let Some(a) = self.editor.archives_mut().get_mut(i)
                    {
                        a.sort_chain = draft.clone();
                        a.sync_sort_state_from_chain();
                        let filter = self.search.clone();
                        a.update_selected_list(&filter, self.config.literal_file_types);
                    }
                    self.config.default_sort_chain = draft.clone();
                    self.editor.set_default_sort_chain(draft);
                    self.save_config();
                }
                self.show_sort_manager = false;
                Task::none()
            }
            Message::SortAddSlot => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    // Add a disabled Size ASC slot as a neutral
                    // default — the user almost always wants to
                    // change the key after adding.
                    draft.push(crate::sort::SortPriority::new(
                        crate::sort::SortKey::Size,
                        crate::sort::SortDirection::Ascending,
                    ));
                }
                Task::none()
            }
            Message::SortRemoveSlot(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.remove(index);
                }
                Task::none()
            }
            Message::SortMoveSlotUp(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut()
                    && index > 0
                {
                    draft.move_slot(index, index - 1);
                }
                Task::none()
            }
            Message::SortMoveSlotDown(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.move_slot(index, index + 1);
                }
                Task::none()
            }
            Message::SortToggleSlotEnabled(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.toggle_enabled(index);
                }
                Task::none()
            }
            Message::SortSetSlotKey(SortSlotIndex(index), key) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.set_key(index, key);
                }
                Task::none()
            }
            Message::SortSetSlotDirection(SortSlotIndex(index), direction) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.set_direction(index, direction);
                }
                Task::none()
            }
            Message::SortSelectPreset(preset) => {
                // Built-in presets — each is a one-line chain shape
                // the user can further edit. Direction is encoded in
                // the variant so a single call site handles them all.
                if let Some(draft) = self.sort_draft.as_mut() {
                    *draft = preset.to_chain();
                }
                Task::none()
            }

            // ---- Drag-and-drop between archives ----
            // Rust-flavored approach (vs IMGF's MFC OLE):
            //   - The whole drag state lives in `App::drag_state` as
            //     a Copy value, so the borrow checker enforces
            //     consistent update with no heap pointers.
            //   - We never copy entry data on press — the source
            //     archive stays open in the Editor, and the move
            //     handler reads + re-imports through the standard
            //     parser pipeline.
            //   - The release handler either commits (target != source)
            //     or simply drops the state. No try/finally needed
            //     for cleanup; `Option::take` is the cleanup.
            Message::ArchiveDragStarted { source } => {
                if let Some(archive) = self.editor.archives().get(source) {
                    let selected: Vec<usize> = archive
                        .selected_indices
                        .iter()
                        .copied()
                        .filter(|&i| i < archive.entries.len())
                        .collect();
                    if !selected.is_empty() {
                        self.drag_state = Some(crate::ui::drag::DragState::new(source, &selected));
                    }
                }
                Task::none()
            }
            Message::ArchiveDragMoved { over } => {
                if let Some(state) = self.drag_state.as_mut() {
                    state.hover_target = over;
                }
                Task::none()
            }
            Message::ArchiveDragReleased => {
                // Take the state so cancel-on-anything-else is just
                // `self.drag_state = None` (no drop glue needed).
                let Some(state) = self.drag_state.take() else {
                    return Task::none();
                };
                if !state.has_valid_target() {
                    // Drop on the source or empty space = cancel.
                    self.toast = Some("Drag cancelled".to_string());
                    return Task::none();
                }
                let Some(target) = state.hover_target else {
                    return Task::none();
                };
                self.move_entries_between_archives(state.source, target, state.indices());
                Task::none()
            }
            Message::ArchiveDragCancelled => {
                self.drag_state = None;
                self.toast = Some("Drag cancelled".to_string());
                Task::none()
            }
        }
    }

    /// Move a slice of entries from `source` to `target` archive.
    /// The entries are cloned (their data + flags) so the move is
    /// in-memory and reversible; we don't touch the disk. The
    /// source entries are removed by index, which avoids issues
    /// with renamed/removed indices after a move.
    fn move_entries_between_archives(
        &mut self,
        source: usize,
        target: usize,
        entry_indices: &[usize],
    ) {
        if source == target {
            return;
        }

        let archive_count = self.editor.archives().len();
        if source >= archive_count || target >= archive_count {
            return;
        }

        let search = self.search.clone();

        // We need to collect the entries first because we'd
        // otherwise borrow the source archive mutably while also
        // needing to mutate the target archive. Two-phase move
        // avoids the borrow conflict.
        let (source_name, target_name, valid_indices, entries) = {
            let archives = self.editor.archives();
            let source_archive = &archives[source];
            let source_name = source_archive.file_name.clone();
            let target_name = archives[target].file_name.clone();

            let mut valid_indices: Vec<usize> = entry_indices
                .iter()
                .copied()
                .filter(|&index| index < source_archive.entries.len())
                .collect();
            valid_indices.sort_unstable();
            valid_indices.dedup();

            let entries: Vec<crate::archive::EntryInfo> = valid_indices
                .iter()
                .map(|&index| source_archive.entries[index].clone())
                .collect();

            (source_name, target_name, valid_indices, entries)
        };
        if entries.is_empty() {
            return;
        }

        let moved_count = entries.len();
        let selected_archive = self.editor.selected_archive();
        let selected_entry = self.editor.selected_entry();
        let selected_entry_after_move = if selected_archive == Some(source) {
            selected_entry.and_then(|index| {
                if valid_indices.binary_search(&index).is_ok() {
                    None
                } else {
                    let shift = valid_indices
                        .iter()
                        .take_while(|&&removed| removed < index)
                        .count();
                    Some(index.saturating_sub(shift))
                }
            })
        } else {
            selected_entry
        };

        // Generation invalidation prevents an entry index from resolving to
        // a stale preview after the source list changes. Evicting both names
        // as well releases old scene allocations immediately instead of
        // waiting for the byte-budgeted cache to pressure them out.
        self.drop_scene_cache_for_archive(&source_name);
        self.drop_scene_cache_for_archive(&target_name);

        // Insert into the target archive. If the target already
        // has an entry with the same name, we rename the moved
        // copy to avoid silent overwrites. IMGF's drag-and-drop
        // does the same on conflict — we get it for free here.
        if let Some(target_archive) = self.editor.archives_mut().get_mut(target) {
            for mut entry in entries {
                if target_archive
                    .entries
                    .iter()
                    .any(|e| e.file_name == entry.file_name)
                {
                    // Append ".bak" to disambiguate. We could
                    // prompt the user, but the IMGF behaviour
                    // is "just do it", so we follow that lead.
                    entry.file_name =
                        compact_str::CompactString::from(format!("{}.bak", entry.file_name));
                    entry.file_name_lower = entry.file_name.to_ascii_lowercase();
                }
                target_archive.entries.push(entry);
            }
            target_archive.dirty = true;
            target_archive.invalidate_entry_caches();
            target_archive.update_selected_list(&search, self.config.literal_file_types);
        }

        // Remove from the source. We do this in reverse index order
        // so earlier removals don't shift the indices of later
        // removals. Rebuilding the filtered list below also repairs
        // the display-row lookup after raw entry indices shift.
        if let Some(source_archive) = self.editor.archives_mut().get_mut(source) {
            for &index in valid_indices.iter().rev() {
                source_archive.entries.remove(index);
            }
            source_archive.dirty = true;
            source_archive.invalidate_entry_caches();
            source_archive.update_selected_list(&search, self.config.literal_file_types);
        }

        if selected_archive == Some(source) {
            self.editor.set_selected_entry(selected_entry_after_move);
        }

        if let Some((archive_index, entry_index)) = self.active_viewer_entry
            && archive_index == source
        {
            if valid_indices.binary_search(&entry_index).is_ok() {
                self.active_viewer_entry = None;
                self.clear_viewer_load();
                self.viewer3d_handle.clear();
                self.reset_texture_preview_state();
            } else {
                let shift = valid_indices
                    .iter()
                    .take_while(|&&removed| removed < entry_index)
                    .count();
                self.active_viewer_entry = Some((source, entry_index.saturating_sub(shift)));
            }
        }

        if selected_archive == Some(source) {
            self.inspected_entry = self.inspected_entry.take().and_then(|(index, inspection)| {
                if valid_indices.binary_search(&index).is_ok() {
                    None
                } else {
                    let shift = valid_indices
                        .iter()
                        .take_while(|&&removed| removed < index)
                        .count();
                    Some((index.saturating_sub(shift), inspection))
                }
            });
        }

        self.context_menu = None;
        self.toast = Some(format!(
            "Moved {} entries to archive #{}",
            moved_count,
            target + 1
        ));
    }

    fn decode_texture_entry(&self, entry_index: usize) -> Task<Message> {
        let Some(archive_index) = self.editor.selected_archive() else {
            return Task::none();
        };
        let (entry_clone, archive_path, archive_entries, texture_cache) = {
            let Some(archive) = self.editor.archives().get(archive_index) else {
                return Task::none();
            };
            let Some(entry) = archive.entries.get(entry_index) else {
                return Task::none();
            };
            (
                entry.clone(),
                archive.path.clone(),
                archive.entries.clone(),
                Arc::clone(&archive.texture_cache),
            )
        };

        Task::perform(
            async move {
                // Single-flight: the cache hands the placeholder guard to
                // exactly one decoder. A zero-timeout turn into `Timeout`
                // when a decode is already running, and `Value` when the
                // entry was decoded meanwhile — both skip the work.
                match texture_cache.get_value_or_guard(&entry_index, Some(Duration::ZERO)) {
                    GuardResult::Value(_) | GuardResult::Timeout => Message::Noop,
                    GuardResult::Guard(guard) => {
                        let result = tokio::task::spawn_blocking(
                            move || -> Result<Vec<DecodedTexture>, String> {
                                let data = crate::parser::read_entry_data_from_source(
                                    &entry_clone,
                                    archive_path.as_deref(),
                                ).map_err(|e| format!("Failed to read entry: {e}"))?;
                                let extension = std::path::Path::new(entry_clone.file_name.as_str())
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .unwrap_or_default()
                                    .to_ascii_lowercase();

                                match extension.as_str() {
                                    "txd" => {
                                        let txd = crate::parser::txd::parse_txd(&data)
                                            .map_err(|e| format!("TXD parse failed: {e}"))?;

                                        let mut decoded = Vec::new();
                                        for tex in &txd.textures {
                                            let rgba = tex
                                                .decode_rgba()
                                                .map_err(|e| format!("Texture decode failed: {e}"))?;
                                            decoded.push(DecodedTexture {
                                                name: tex.diffuse_name.clone(),
                                                width: tex.width,
                                                height: tex.height,
                                                palette_colors:
                                                    crate::parser::texture_decoder::palette_colors(
                                                        &rgba,
                                                    ),
                                                rgba,
                                                has_alpha: tex.has_alpha_channel(),
                                                format_name: tex.format_name().to_string(),
                                                mipmap_count: tex.num_mipmaps as u32,
                                                raster: Some(
                                                    crate::compat::raster::RasterProfile::from_native(
                                                        tex,
                                                    ),
                                                ),
                                                nif_format: None,
                                                handle: std::sync::OnceLock::new(),
                                            });
                                        }
                                        Ok(decoded)
                                    }
                                    "nft" => {
                                        let archive_texture_index =
                                            crate::inspector::texture::ArchiveTextureIndex::from_entries(
                                                &archive_entries,
                                                archive_path.as_deref(),
                                            );
                                        crate::inspector::texture::decode_nft_textures_with_resolver(
                                            &data,
                                            |source_path| archive_texture_index.read(source_path),
                                        )
                                    }
                                    _ => Err(format!(
                                        "Texture preview supports TXD and NFT entries; '{}' is not a supported texture container.",
                                        entry_clone.file_name
                                    )),
                                }
                            },
                        )
                        .await;

                        match result.unwrap_or_else(|e| Err(format!("task panicked: {e}"))) {
                            Ok(textures) => {
                                let textures = Arc::new(textures);
                                // Publishing through the placeholder keeps the
                                // dedup atomic with the insert. An `Err` means
                                // the cache was invalidated mid-decode (entries
                                // changed), so the decode is stale - drop it.
                                if guard.insert(Arc::clone(&textures)).is_err() {
                                    return Message::Noop;
                                }
                                Message::TextureDecoded {
                                    archive_index,
                                    index: entry_index,
                                    result: Ok(textures),
                                }
                            }
                            Err(err) => {
                                // Dropping the guard releases the placeholder so
                                // the user can retry the decode.
                                drop(guard);
                                Message::TextureDecoded {
                                    archive_index,
                                    index: entry_index,
                                    result: Err(err),
                                }
                            }
                        }
                    }
                }
            },
            |message| message,
        )
    }

    fn start_export(&mut self, mode: ExportMode) -> Task<Message> {
        self.last_export_selected_only = matches!(mode, ExportMode::Selected);
        self.toast = None;
        dialogs::save_folder().map(Message::ExportFolderResult)
    }

    fn poll_viewer_rxs(&mut self) {
        let mut logs: Vec<String> = Vec::new();
        let mut toast: Option<String> = None;
        self.viewer_rxs.retain_mut(|rx| {
            loop {
                match rx.try_recv() {
                    Ok(ViewerEvent::Opened { name }) => {
                        logs.push(format!("3D viewer opened: {name}"));
                    }
                    Ok(ViewerEvent::Failed { reason }) => {
                        toast = Some(reason.clone());
                        logs.push(format!("3D viewer failed: {reason}"));
                    }
                    Ok(ViewerEvent::Closed) => {
                        logs.push("3D viewer closed".to_string());
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => return false,
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break true,
                }
            }
        });
        if let Some(msg) = toast {
            self.toast = Some(msg);
        }
        if let Some(archive) = self.editor.selected_archive_mut() {
            for log in logs {
                archive.add_log(log);
            }
        }
    }

    /// Hide the native window before asking Iced to release its compositor.
    /// `set_mode` is an effect, so chaining the settle delay guarantees that
    /// the close action cannot race the visibility request.
    fn hide_window_then_close(&mut self, window: iced::window::Id) -> Task<Message> {
        self.quitting = Some(window);
        iced::window::set_mode::<Message>(window, iced::window::Mode::Hidden).chain(
            Task::perform(
                async {
                    tokio::time::sleep(QUIT_HIDE_SETTLE).await;
                },
                |_| Message::QuitWindowHidden,
            ),
        )
    }
}

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        // Track modifier keys from ALL keyboard events (press + release).
        let mod_tracker = iced::event::listen().map(|event| match event {
            iced::Event::Keyboard(ke) => match ke {
                KeyboardEvent::KeyPressed { modifiers, .. }
                | KeyboardEvent::KeyReleased { modifiers, .. }
                | KeyboardEvent::ModifiersChanged(modifiers) => {
                    Message::ModifiersChanged(modifiers)
                }
            },
            // Only UNCAPTURED presses reach this listener. Widgets that
            // own a press (text input, prediction buttons, entry rows)
            // capture it and handle it themselves, so an uncaptured
            // press is a click on inert space — the right moment to
            // dismiss the floating search dropdown.
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Left,
            )) => Message::UncapturedPress,
            _ => Message::Noop,
        });

        let key = iced::keyboard::listen().map(|event| match event {
            KeyboardEvent::KeyPressed {
                physical_key,
                modifiers,
                ..
            } => detect_pressed(physical_key, modifiers)
                .map(Message::ShortcutPressed)
                .unwrap_or(Message::Noop),
            _ => Message::Noop,
        });

        // Search-prediction keyboard navigation. These fire on every key
        // press regardless of focus; the update handlers no-op unless
        // the prediction dropdown is actually open.
        let search_keys = iced::event::listen_with(|event, _status, _window| {
            match event {
                iced::Event::Keyboard(KeyboardEvent::KeyPressed { key, .. }) => match key {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                        Some(Message::SearchPredictMove(-1))
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                        Some(Message::SearchPredictMove(1))
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                        Some(Message::SearchPredictCommit)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        Some(Message::SearchPredictDismiss)
                    }
                    _ => None,
                },
                _ => None,
            }
        });

        let tick = iced::time::every(Duration::from_millis(250)).map(|_| Message::TickProgress);

        // Only run the animation ticker when something needs it. A constant
        // 60 Hz update forces a full view rebuild every frame, which makes
        // scrolling and typing feel sluggish on large archives. The
        // empty-archive idle screen is cheap to redraw, so the breathing
        // icon may keep the ticker alive there.
        let anim_tick = if self.animator.running_count() > 0
            || self.toast.is_some()
            || self.toast_reveal_text.is_some()
            || self.viewer_load.is_some()
            || self.has_active_progress()
            || self.autoscroll_momentum.is_active()
            || (self.editor.archives().is_empty() && self.config.motion_enabled)
        {
            iced::time::every(Duration::from_millis(16)).map(Message::AnimationTick)
        } else {
            Subscription::none()
        };

        let debounce = iced::time::every(Duration::from_millis(150)).map(|_| Message::DebounceTick);

        let window = iced::window::events().map(|(id, event)| match event {
            iced::window::Event::FileDropped(path) => Message::FilesDropped(path),
            iced::window::Event::CloseRequested => Message::WindowCloseRequested(id),
            _ => Message::Noop,
        });

        let autoscroll_start = iced::event::listen_with(|event, status, _window| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::PointerMoved(position))
            }
            // The native Scrollable captures a valid MMB autoscroll request.
            // Only then mirror its state and show the notice; an MMB click on
            // another control must not claim that table autoscroll started.
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Middle,
            )) if matches!(status, iced::event::Status::Captured) => {
                Some(Message::AutoScrollStarted)
            }
            _ => None,
        });

        let autoscroll_stop = if self.autoscroll {
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button))
                    if button != iced::mouse::Button::Middle =>
                {
                    Some(Message::AutoScrollEnded)
                }
                iced::Event::Keyboard(KeyboardEvent::KeyPressed {
                    key:
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::AutoScrollEscape),
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled { .. })
                | iced::Event::Keyboard(_) => Some(Message::AutoScrollEnded),
                _ => None,
            })
        } else {
            Subscription::none()
        };

        // While the tab divider is grabbed, track the cursor globally so
        // the drag keeps working once the pointer leaves the narrow grip.
        let tab_resize = if self.tab_resize_drag.is_some() {
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::TabResizeMoved(position.x))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::TabResizeEnded),
                _ => None,
            })
        } else {
            Subscription::none()
        };

        Subscription::batch([
            mod_tracker,
            key,
            search_keys,
            tick,
            anim_tick,
            debounce,
            window,
            autoscroll_start,
            autoscroll_stop,
            tab_resize,
        ])
    }
}

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        crate::ui::view::build(self)
    }

    pub fn menubar(&self) -> Element<'_, Message> {
        // The "Open Recent" submenu is built from `iter_existing` so
        // dead links vanish without mutating the stored MRU list.
        // An empty list renders a single disabled "No recent files"
        // item so the user can see why the menu is empty.
        let recent_menu_items: Vec<Item<'_, Message, _, _>> =
            if self.config.recent_files.iter_existing().next().is_none() {
                vec![Item::new(iced::Element::from(
                    iced::widget::text("No recent files").size(13),
                ))]
            } else {
                self.config
                    .recent_files
                    .iter()
                    .map(|(index, entry)| {
                        let label = self.config.recent_files.menu_label(index, 60);
                        let path = entry.path.clone();
                        Item::new(menu_button(label, Message::OpenRecent(path)))
                    })
                    .collect()
            };
        let recent_menu = Menu::new(recent_menu_items).max_width(320.0);

        let file_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("New ({})", shortcut_display(Shortcut::New)),
                Message::NewArchive,
            )),
            Item::new(menu_button(
                format!("Open… ({})", shortcut_display(Shortcut::Open)),
                Message::OpenArchive,
            )),
            Item::with_menu(
                menu_button_with_icon(
                    "Open Recent".to_string(),
                    icons::open_archive().size(16).into(),
                    Message::Noop,
                ),
                recent_menu,
            ),
            Item::new(menu_button(
                format!("Save ({})", shortcut_display(Shortcut::Save)),
                Message::SaveArchive,
            )),
            Item::new(menu_button(
                format!("Save as… ({})", shortcut_display(Shortcut::SaveAs)),
                Message::SaveArchiveAs,
            )),
            Item::new(menu_button(
                "Pack archive".to_string(),
                Message::PackArchive,
            )),
            Item::new(menu_button(
                format!("Close tab ({})", shortcut_display(Shortcut::Close)),
                Message::CloseSelectedArchive,
            )),
            Item::new(menu_button(
                "Sort by…".to_string(),
                Message::OpenSortManager,
            )),
        ])
        .max_width(220.0);

        let edit_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("Import ({})", shortcut_display(Shortcut::Import)),
                Message::ImportFiles,
            )),
            Item::new(menu_button(
                "Import folder".to_string(),
                Message::ImportFolder,
            )),
            Item::new(menu_button(
                format!("Export all ({})", shortcut_display(Shortcut::ExportAll)),
                Message::ExportAll,
            )),
            Item::new(menu_button(
                format!(
                    "Export selected ({})",
                    shortcut_display(Shortcut::ExportSelected)
                ),
                Message::ExportSelected,
            )),
        ])
        .max_width(220.0);

        let selection_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("Select all ({})", shortcut_display(Shortcut::SelectAll)),
                Message::SelectAll,
            )),
            Item::new(menu_button(
                format!(
                    "Invert selection ({})",
                    shortcut_display(Shortcut::InvertSelection)
                ),
                Message::InvertSelection,
            )),
            Item::new(menu_button(
                format!(
                    "Clear selection ({})",
                    shortcut_display(Shortcut::ClearSelection)
                ),
                Message::ClearSelection,
            )),
            Item::new(menu_button(
                format!("Delete selected ({})", shortcut_display(Shortcut::Delete)),
                Message::DeleteSelected,
            )),
        ])
        .max_width(220.0);

        let option_items: Vec<Item<'_, Message, iced::Theme, iced::Renderer>> = ThemeMode::ALL
            .iter()
            .map(|mode| {
                let label = if *mode == self.config.theme {
                    format!("● {}", mode.as_str())
                } else {
                    format!("○ {}", mode.as_str())
                };
                Item::new(menu_button(label, Message::SetTheme(*mode)))
            })
            .collect();

        let option_menu = Menu::new(option_items).max_width(220.0);

        // The View menu contains application-wide interaction preferences.
        let view_toggle = |on: bool| if on { "● " } else { "○ " };
        let view_menu = Menu::new(vec![
            Item::new(menu_button(
                format!(
                    "Go to export tab ({})",
                    shortcut_display(Shortcut::SwitchTab(InspectorTab::Export))
                ),
                Message::Viewer3dSelectTab(InspectorTab::Export),
            )),
            Item::new(menu_button(
                format!(
                    "Go to 3D viewer ({})",
                    shortcut_display(Shortcut::SwitchTab(InspectorTab::Model3D))
                ),
                Message::Viewer3dSelectTab(InspectorTab::Model3D),
            )),
            Item::new(menu_button(
                format!(
                    "Go to texture viewer ({})",
                    shortcut_display(Shortcut::SwitchTab(InspectorTab::Texture))
                ),
                Message::Viewer3dSelectTab(InspectorTab::Texture),
            )),
            Item::new(menu_button(
                format!(
                    "{}Navigation gizmo",
                    view_toggle(self.config.show_navigation_gizmo)
                ),
                Message::SetNavigationGizmoVisible(!self.config.show_navigation_gizmo),
            )),
            Item::new(menu_button(
                format!(
                    "{}Search bar",
                    view_toggle(self.config.show_search_bar)
                ),
                Message::ToggleSearchBar(!self.config.show_search_bar),
            )),
            Item::new(menu_button(
                format!(
                    "{}Literal file types",
                    view_toggle(self.config.literal_file_types)
                ),
                Message::ToggleLiteralFileTypes(!self.config.literal_file_types),
            )),
            Item::new(menu_button(
                format!(
                    "{}Highlight validator rows",
                    view_toggle(self.compat_highlight_enabled)
                ),
                Message::SetCompatHighlight(!self.compat_highlight_enabled),
            )),
            Item::new(menu_button(
                format!(
                    "{}Right-click adds to selection",
                    view_toggle(self.config.context_selection_accumulates)
                ),
                Message::ToggleContextAccumulate(!self.config.context_selection_accumulates),
            )),
            Item::new(menu_button(
                format!(
                    "{}Autoscroll momentum",
                    view_toggle(self.config.autoscroll_momentum_enabled)
                ),
                Message::ToggleAutoscrollMomentum(!self.config.autoscroll_momentum_enabled),
            )),
            Item::new(menu_button(
                format!(
                    "{}Motion effects",
                    view_toggle(self.config.motion_enabled)
                ),
                Message::ToggleMotionEffects(!self.config.motion_enabled),
            )),
            Item::new(menu_button(
                format!(
                    "{}Selection pulse",
                    view_toggle(self.config.selection_pulse_enabled)
                ),
                Message::ToggleSelectionPulse(!self.config.selection_pulse_enabled),
            )),
            Item::new(menu_button(
                format!(
                    "{}Click ripples",
                    view_toggle(self.config.click_ripple_enabled)
                ),
                Message::ToggleClickRipple(!self.config.click_ripple_enabled),
            )),
            Item::new(menu_button(
                format!(
                    "{}Icon micro-motion",
                    view_toggle(self.config.icon_micro_motion_enabled)
                ),
                Message::ToggleIconMicroMotion(!self.config.icon_micro_motion_enabled),
            )),
        ])
        .max_width(220.0);

        let help_menu = Menu::new(vec![
            Item::new(menu_button(
                format!(
                    "Check for updates ({})\u{200B}",
                    shortcut_display(Shortcut::CheckUpdates)
                ),
                Message::CheckUpdatesManual,
            )),
            Item::new(menu_button(
                "Visit repository\u{200B}".to_string(),
                Message::VisitRepository,
            )),
            Item::new(menu_button("About".to_string(), Message::ShowAbout)),
        ])
        .max_width(220.0);

        fn menu_label(label: &'static str) -> iced::Element<'static, Message> {
            container(fonts::header(label)).padding([4, 12]).into()
        }

        let bar = MenuBar::new(vec![
            Item::with_menu(menu_label("File"), file_menu),
            Item::with_menu(menu_label("Edit"), edit_menu),
            Item::with_menu(menu_label("Selection"), selection_menu),
            Item::with_menu(menu_label("View"), view_menu),
            Item::with_menu(menu_label("Themes"), option_menu),
            Item::with_menu(menu_label("Help"), help_menu),
        ]);

        let design = self.design();
        let (top, bottom) = design.menubar_gradient();
        let border = design.border();
        iced::widget::Container::new(bar)
            .width(iced::Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(0.0)
                        .add_stop(0.0, top)
                        .add_stop(1.0, bottom),
                ))),
                border: iced::Border {
                    color: border,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

fn open_export_folder(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn menu_button<'a>(label: String, message: Message) -> Element<'a, Message> {
    menu_button_with_icon(label, menu_icon(&message), message)
}

pub(crate) fn format_byte_count(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_folder_import_summary(summary: &FolderImportSummary) -> String {
    let prefix = if summary.cancelled {
        "Folder import cancelled"
    } else {
        "Folder import complete"
    };
    let details = if summary.failed > 0 || !summary.details.is_empty() {
        " See the archive log for details."
    } else {
        ""
    };
    format!(
        "{prefix}: {} imported, {} skipped, {} failed.{details}",
        summary.imported, summary.skipped, summary.failed
    )
}

fn menu_button_with_icon<'a>(
    label: String,
    icon: Element<'a, Message>,
    message: Message,
) -> Element<'a, Message> {
    iced::widget::button(w::icon_label(
        icon,
        fonts::body(label)
            .align_x(iced::alignment::Horizontal::Left)
            .width(iced::Length::Fill),
    ))
    .on_press(message)
    .width(iced::Length::Fill)
    .style(
        |theme: &iced::Theme, status: iced::widget::button::Status| {
            let palette = theme.extended_palette();
            let highlighted = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            let highlight = palette.background.strong.color;
            iced::widget::button::Style {
                background: if highlighted {
                    Some(highlight.into())
                } else {
                    None
                },
                text_color: if highlighted {
                    w::readable_text_color(highlight, palette.background.base.text)
                } else {
                    palette.background.base.text
                },
                ..iced::widget::button::Style::default()
            }
        },
    )
    .into()
}

fn menu_icon(message: &Message) -> Element<'static, Message> {
    let icon = match message {
        Message::NewArchive => icons::new_archive(),
        Message::OpenArchive | Message::OpenRecent(_) => icons::open_archive(),
        Message::SaveArchive | Message::SaveArchiveAsResult(_) | Message::SaveArchiveAs => {
            icons::save()
        }
        Message::PackArchive => icons::pack(),
        Message::CloseSelectedArchive => icons::close(),
        Message::OpenSortManager => icons::sort(),
        Message::ImportFiles => icons::import(),
        Message::ImportFolder => icons::open_archive(),
        Message::ExportAll | Message::ExportSelected => icons::export(),
        Message::SelectAll => icons::check(),
        Message::InvertSelection => icons::invert_selection(),
        Message::ClearSelection => icons::close(),
        Message::DeleteSelected => icons::delete(),
        Message::SetTheme(_) => icons::settings(),
        Message::CheckUpdatesManual | Message::ShowAbout => icons::help(),
        Message::VisitRepository => icons::external_viewer(),
        _ => icons::generic_file(),
    };
    icon.size(16).into()
}

pub fn run_app(config: Config) -> iced::Result {
    let size: iced::Size = config.window.size.unwrap_or([1100.0, 720.0]).into();

    let boot_config = Arc::new(config);
    let boot_config_for_boot = Arc::clone(&boot_config);

    iced::application(
        move || {
            let cfg = (*boot_config_for_boot).clone();
            (App::new(cfg.clone()), App::startup_task(&cfg))
        },
        App::update,
        App::view,
    )
    .title(|_: &App| "IMG Editor Plus".to_string())
    .theme(|state: &App| -> Option<Theme> { Some(state.theme()) })
    .subscription(App::subscription)
    .settings(iced::Settings {
        default_text_size: iced::Pixels(14.0),
        fonts: vec![
            crate::ui::fonts::INTER_FONT_BYTES.into(),
            crate::ui::fonts::BRICOLAGE_DISPLAY_FONT_BYTES.into(),
        ],
        ..iced::Settings::default()
    })
    .window(iced::window::Settings {
        icon: window_icon(),
        // The close button must not quit on its own: dirty archives get
        // an "unsaved changes" confirmation, and only then do we close
        // the window ourselves.
        exit_on_close_request: false,
        ..iced::window::Settings::default()
    })
    .default_font(crate::ui::fonts::INTER)
    .window_size(size)
    .resizable(true)
    .centered()
    .run()
}

fn window_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../../asset/logo/IMGEditorLogo.png");
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let image = image.to_rgba8();
    let (width, height) = image.dimensions();
    iced::window::icon::from_rgba(image.into_raw(), width, height).ok()
}

#[allow(dead_code)]
fn _force_space_use(_: Space) {}

/// Encoder options for the interactive converter dialogs: the
/// high-quality DXT flag maps onto the iterative cluster fit.
fn replace_encode_options(high_quality: bool) -> crate::compat::encode::EncodeOptions {
    crate::compat::encode::EncodeOptions {
        dxt_quality: if high_quality {
            crate::compat::encode::DxtQuality::High
        } else {
            crate::compat::encode::DxtQuality::Standard
        },
        dither: false,
    }
}

/// Plan one texture replacement off the UI thread.
#[allow(clippy::too_many_arguments)]
fn plan_replace(
    entry: &crate::archive::EntryInfo,
    archive_path: Option<&std::path::Path>,
    archive_index: usize,
    entry_index: usize,
    texture_index: usize,
    path: PathBuf,
    target: &'static crate::compat::games::GameProfile,
    archive_name: &str,
    format: Option<crate::compat::encode::EncodeFormat>,
    options: crate::compat::encode::EncodeOptions,
) -> Result<ReplacePlanReady, String> {
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("read {} failed: {error}", path.display()))?;
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();
    let image = crate::compat::convert::decode_source_image_named(&bytes, &source_name)?;
    let entry_bytes = crate::parser::read_entry_data_from_source(entry, archive_path)
        .map_err(|error| error.to_string())?;
    let txd = crate::parser::txd::parse_txd(&entry_bytes)?;
    let old = txd
        .textures
        .get(texture_index)
        .ok_or_else(|| "this entry's texture list changed; reopen it".to_string())?;
    let before_rgba = old.decode_rgba().map_err(|error| error.to_string())?;
    let plan =
        crate::compat::convert::plan_import(&image, target, archive_name, format, options)?;
    Ok(ReplacePlanReady {
        archive_index,
        entry_index,
        texture_index,
        source_path: path,
        source_name,
        texture_name: old.diffuse_name.clone(),
        before: Some((old.width, old.height, Arc::new(before_rgba))),
        plan: CompactPlan(Arc::new(plan)),
        entry: entry.clone(),
        archive_path: archive_path.map(std::path::Path::to_path_buf),
        target,
        archive_name: archive_name.to_string(),
    })
}

/// Plan a new single-texture TXD from an image, off the UI thread.
fn plan_txd_import(
    archive_index: usize,
    path: PathBuf,
    target: &'static crate::compat::games::GameProfile,
    archive_name: &str,
    format: Option<crate::compat::encode::EncodeFormat>,
    options: crate::compat::encode::EncodeOptions,
) -> Result<NewTxdPlanReady, String> {
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("read {} failed: {error}", path.display()))?;
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();
    let image = crate::compat::convert::decode_source_image_named(&bytes, &source_name)?;
    let texture_name = std::path::Path::new(&source_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("texture")
        .to_string();
    let plan =
        crate::compat::convert::plan_import(&image, target, archive_name, format, options)?;
    Ok(NewTxdPlanReady {
        archive_index,
        source_path: path,
        source_name,
        texture_name,
        target,
        plan: CompactPlan(Arc::new(plan)),
    })
}

/// Plan converting every texture of the selected entries to the target
/// dialect, off the UI thread. Textures already native are skipped.
fn plan_bulk_convert(
    archive_index: usize,
    archive: &crate::archive::ArchiveInfo,
    selected: &[usize],
    target: &'static crate::compat::games::GameProfile,
) -> Result<BulkPlanReady, String> {
    use crate::compat::games::{classify, Verdict};
    use crate::compat::raster::RasterProfile;

    let mut entries = Vec::new();
    for &index in selected {
        let Some(entry) = archive.entries.get(index) else {
            continue;
        };
        if !entry.file_name_lower.ends_with(".txd") {
            continue;
        }
        let Ok(bytes) = crate::parser::read_entry_data(archive, entry) else {
            continue;
        };
        let Ok(txd) = crate::parser::txd::parse_txd(&bytes) else {
            continue;
        };
        let mut textures = Vec::new();
        let mut skipped_native = 0usize;
        let mut failed = 0usize;
        for (texture_index, texture) in txd.textures.iter().enumerate() {
            let profile = RasterProfile::from_native(texture);
            if classify(target, &profile).verdict == Verdict::Native {
                skipped_native += 1;
                continue;
            }
            match crate::compat::convert::plan_conversion(
                &bytes,
                texture_index,
                target,
                &archive.file_name,
                None,
                crate::compat::encode::EncodeOptions::default(),
            ) {
                Ok(plan) => textures.push((
                    texture_index,
                    texture.diffuse_name.clone(),
                    CompactPlan(Arc::new(plan)),
                )),
                Err(_) => failed += 1,
            }
        }
        if !textures.is_empty() {
            entries.push(BulkEntryPlan {
                entry_index: index,
                file_name: entry.file_name.to_string(),
                textures,
                skipped_native,
                failed,
            });
        }
    }
    Ok(BulkPlanReady {
        archive_index,
        source_label: archive.file_name.clone(),
        entries,
    })
}

/// Converted entry bytes, keyed by entry index.
type BulkPatches = Vec<(usize, Arc<Vec<u8>>)>;

/// Execute a bulk-conversion plan off the UI thread.
fn convert_bulk_entries(
    archive: &crate::archive::ArchiveInfo,
    entries: &[BulkEntryPlan],
) -> Result<BulkPatches, String> {
    let mut patches = Vec::new();
    for entry_plan in entries {
        let entry = archive
            .entries
            .get(entry_plan.entry_index)
            .ok_or_else(|| "entry disappeared during conversion".to_string())?;
        let mut bytes =
            crate::parser::read_entry_data(archive, entry).map_err(|error| error.to_string())?;
        for (texture_index, _, plan) in &entry_plan.textures {
            bytes = crate::compat::convert::apply_replace(&bytes, *texture_index, &plan.0)?;
        }
        patches.push((entry_plan.entry_index, Arc::new(bytes)));
    }
    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::EntryInfo;

    fn test_app() -> App {
        let mut app = App::new(Config::default());
        // The first-run welcome modal gates shortcuts now; tests below
        // assume a normal workspace with no dialog open.
        app.show_welcome = false;
        // Never write the user's real settings.ini from a test.
        app.config_persist_enabled = false;
        app
    }

    fn test_app_with_entries() -> App {
        let mut app = test_app();
        app.editor.new_archive();
        let archive = app.editor.archives_mut().first_mut().unwrap();
        archive.entries.push(EntryInfo::new("first.dff"));
        archive.entries.push(EntryInfo::new("second.txd"));
        archive.update_selected_list("", false);
        app
    }

    /// Run a task's side effects and collect the follow-up messages it
    /// would feed back into the runtime (mirrors what the real event loop
    /// does with `Task::done` values).
    fn drain_task(task: iced::task::Task<Message>) -> Vec<Message> {
        use iced::futures::StreamExt;

        let Some(stream) = iced_runtime::task::into_stream(task) else {
            return Vec::new();
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        runtime.block_on(async move {
            stream
                .filter_map(|action| async move {
                    match action {
                        iced_runtime::Action::Output(message) => Some(message),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
                .await
        })
    }

    #[test]
    fn search_changed_updates_text_immediately() {
        let mut app = test_app();
        let _ = app.update(Message::SearchChanged("player".to_string()));
        assert_eq!(app.search, "player");
        assert!(app.filter_pending);
    }

    #[test]
    fn search_changed_no_op_for_same_value() {
        let mut app = test_app();
        app.search = "player".to_string();
        app.filter_pending = false;
        let _ = app.update(Message::SearchChanged("player".to_string()));
        assert!(!app.filter_pending);
    }

    #[test]
    fn debounce_tick_applies_pending_filter() {
        let mut app = test_app();
        app.editor.new_archive();
        app.search = "player".to_string();
        app.filter_pending = true;

        let _ = app.update(Message::DebounceTick);

        assert!(!app.filter_pending);
    }

    #[test]
    fn rename_input_updates_buffer_without_committing() {
        let mut app = test_app();
        let _ = app.update(Message::RenameInputChanged("player".to_string()));
        assert_eq!(app.rename_buffer, "player");
    }

    #[test]
    fn commit_rename_uses_buffer_and_clears_it() {
        let mut app = test_app();
        app.rename_buffer = "renamed".to_string();

        let _ = app.update(Message::CommitRename);

        assert!(app.rename_buffer.is_empty());
    }

    #[test]
    fn context_delete_targets_right_clicked_entry() {
        let mut app = test_app_with_entries();

        let _ = app.update(Message::EntryRightClicked(1));
        assert!(app.editor.archives()[0].entries[1].selected);

        let _ = app.update(Message::EntryContextAction(EntryAction::Delete));

        assert_eq!(app.editor.archives()[0].entries.len(), 1);
        assert_eq!(app.editor.archives()[0].entries[0].file_name, "first.dff");
    }

    #[test]
    fn moving_entries_invalidates_both_preview_caches_and_repairs_indices() {
        use crate::inspector::scene3d::{BaseOrientation, Scene};

        let mut app = test_app();
        app.editor.new_archive();
        {
            let source = app.editor.archives_mut().first_mut().unwrap();
            source.entries.push(EntryInfo::new("a.nif"));
            source.entries.push(EntryInfo::new("b.nif"));
            source.entries.push(EntryInfo::new("c.nif"));
            source.entries[2].selected = true;
            source.update_selected_list("", false);
            source.texture_cache.insert(2, Arc::new(Vec::new()));
        }

        app.editor.new_archive();
        {
            let target = app.editor.archives_mut().get_mut(1).unwrap();
            target.entries.push(EntryInfo::new("target.nif"));
            target.update_selected_list("", false);
            target.texture_cache.insert(0, Arc::new(Vec::new()));
        }

        let source_name = app.editor.archives()[0].file_name.clone();
        let source_generation = app.editor.archives()[0].generation();
        app.scene_cache.insert(
            (source_name, source_generation, 2),
            Arc::new(Scene::empty(BaseOrientation::Yup)),
        );

        app.editor.select_archive(0);
        app.editor.set_selected_entry(Some(2));
        app.active_viewer_entry = Some((0, 2));
        app.viewer3d_handle
            .set_scene(Arc::new(Scene::empty(BaseOrientation::Yup)));

        app.move_entries_between_archives(0, 1, &[0]);

        let source = &app.editor.archives()[0];
        let target = &app.editor.archives()[1];
        assert_eq!(source.entries[0].file_name, "b.nif");
        assert_eq!(source.entries[1].file_name, "c.nif");
        assert!(
            target
                .entries
                .iter()
                .any(|entry| entry.file_name == "a.nif")
        );
        assert!(
            target
                .entries
                .iter()
                .any(|entry| entry.file_name == "target.nif")
        );
        assert_eq!(source.selected_indices.as_slice(), &[0, 1]);
        assert_eq!(source.display_row_of(0), Some(0));
        assert_eq!(source.display_row_of(1), Some(1));
        assert_eq!(app.editor.selected_entry(), Some(1));
        assert_eq!(app.active_viewer_entry, Some((0, 1)));
        assert!(app.viewer3d_handle.with(|inner| inner.scene.is_some()));
        assert_eq!(source.generation(), 1);
        assert_eq!(target.generation(), 1);
        assert!(source.texture_cache.is_empty());
        assert!(target.texture_cache.is_empty());
        assert!(app.scene_cache.is_empty());
        assert!(source.dirty);
        assert!(target.dirty);
    }

    #[test]
    fn open_3d_action_selects_model_tab_and_clears_stale_scene_target() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0]
            .entries
            .push(EntryInfo::new("model.nif"));
        app.editor.archives_mut()[0].update_selected_list("", false);
        app.editor.select_entry(2, false, false);
        app.active_viewer_entry = Some((0, 0));

        let _ = app.update(Message::EntryContextAction(EntryAction::Render));

        assert_eq!(app.selected_inspector_tab, InspectorTab::Model3D);
        assert_eq!(app.active_viewer_entry, None);
        assert!(app.viewer3d_handle.with(|inner| inner.scene.is_none()));
        assert_eq!(
            app.viewer_load.as_ref().map(|load| load.target),
            Some((0, 2))
        );
        assert_eq!(
            app.viewer_load
                .as_ref()
                .map(|load| load.entry_name.as_str()),
            Some("model.nif")
        );
    }

    #[test]
    fn open_texture_action_selects_texture_tab() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(1, false, false);

        let _ = app.update(Message::EntryContextAction(EntryAction::ViewTextures));

        assert_eq!(app.selected_inspector_tab, InspectorTab::Texture);
        assert_eq!(app.selected_texture, 0);
        assert!(!app.show_texture_uv);
    }

    #[test]
    fn alpha_blending_is_enabled_by_default_and_toggleable() {
        let mut app = test_app();
        assert!(app.viewer3d_handle.with(|inner| {
            inner
                .flags
                .contains(crate::inspector::scene3d::pipeline::RenderFlags::ALPHA_BLEND)
        }));

        let _ = app.update(Message::Viewer3dToggleAlphaBlend);
        assert!(!app.viewer3d_handle.with(|inner| {
            inner
                .flags
                .contains(crate::inspector::scene3d::pipeline::RenderFlags::ALPHA_BLEND)
        }));
    }

    #[test]
    fn sort_header_updates_chain_and_visible_order() {
        let mut app = test_app();
        app.editor.new_archive();
        let archive = app.editor.archives_mut().first_mut().unwrap();
        archive.entries.push(EntryInfo::new("small.dff"));
        archive.entries.push(EntryInfo::new("large.dff"));
        archive.entries.push(EntryInfo::new("middle.dff"));
        archive.entries[0].sector = 1;
        archive.entries[1].sector = 9;
        archive.entries[2].sector = 4;
        archive.update_selected_list("", false);

        let _ = app.update(Message::SortBy(SortColumn::Size));
        let archive = &app.editor.archives()[0];
        assert_eq!(archive.sort_chain.iter().next().unwrap().key, SortKey::Size);
        assert_eq!(archive.selected_indices.as_slice(), &[1, 2, 0]);

        let _ = app.update(Message::SortBy(SortColumn::Size));
        let archive = &app.editor.archives()[0];
        assert_eq!(
            archive.sort_chain.iter().next().unwrap().direction,
            SortDirection::Ascending
        );
        assert_eq!(archive.selected_indices.as_slice(), &[0, 2, 1]);

        let _ = app.update(Message::SortBy(SortColumn::Name));
        let archive = &app.editor.archives()[0];
        assert_eq!(archive.sort_chain.iter().next().unwrap().key, SortKey::Name);
        assert_eq!(archive.selected_indices.as_slice(), &[1, 2, 0]);
        assert_eq!(app.config.default_sort_chain, archive.sort_chain);
    }

    #[test]
    fn configured_sort_chain_is_inherited_by_new_archives() {
        let config = Config {
            default_sort_chain: SortChain::new(vec![SortPriority::new(
                SortKey::Size,
                SortDirection::Descending,
            )]),
            ..Config::default()
        };
        let mut app = App::new(config);

        app.editor.new_archive();

        let archive = &app.editor.archives()[0];
        assert_eq!(archive.sort_chain.iter().next().unwrap().key, SortKey::Size);
        assert_eq!(archive.sort.column, SortColumn::Size);
    }

    #[test]
    fn clicking_nif_while_viewing_3d_starts_the_selected_scene() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0]
            .entries
            .push(EntryInfo::new("model.nif"));
        app.editor.archives_mut()[0].update_selected_list("", false);
        app.selected_inspector_tab = InspectorTab::Model3D;
        app.active_viewer_entry = Some((0, 0));
        let row = app.editor.archives()[0]
            .selected_indices
            .iter()
            .position(|&index| index == 2)
            .expect("model row");

        let _ = app.update(Message::EntryClicked(row));

        assert_eq!(app.editor.selected_entry(), Some(2));
        assert_eq!(app.selected_inspector_tab, InspectorTab::Model3D);
        assert_eq!(app.active_viewer_entry, None);
        assert_eq!(
            app.viewer_load.as_ref().map(|load| load.target),
            Some((0, 2))
        );
    }

    #[test]
    fn viewer_loading_phase_advances_only_while_the_selected_model_is_pending() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        assert_eq!(app.selected_entry_key(), Some((0, 0)));
        app.begin_viewer_load((0, 0), "first.dff".to_string());
        let start = std::time::Instant::now();

        let _ = app.update(Message::AnimationTick(start));
        let _ = app.update(Message::AnimationTick(start + Duration::from_millis(250)));

        assert!(
            app.viewer_load_phase > 0.0,
            "phase: {}, load state: {:?}",
            app.viewer_load_phase,
            app.viewer_load
        );
        assert!(app.viewer_load_phase < 1.0);
    }

    #[test]
    fn stale_viewer_completion_does_not_hide_the_newer_model_transition() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0]
            .entries
            .push(EntryInfo::new("newer.nif"));
        app.editor.archives_mut()[0].update_selected_list("", false);
        app.editor.select_entry(2, false, false);
        app.begin_viewer_load((0, 2), "newer.nif".to_string());

        let _ = app.update(Message::Viewer3dLoadCompleted {
            archive_index: 0,
            entry_index: 0,
            generation: 0,
            result: Err("stale completion".to_string()),
            ide_map: None,
        });

        assert_eq!(
            app.viewer_load.as_ref().map(|load| load.target),
            Some((0, 2))
        );
    }

    #[test]
    fn matching_viewer_completion_clears_the_loading_transition() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        app.begin_viewer_load((0, 0), "first.dff".to_string());

        let _ = app.update(Message::Viewer3dLoadCompleted {
            archive_index: 0,
            entry_index: 0,
            generation: 0,
            result: Err("intentional failure".to_string()),
            ide_map: None,
        });

        assert!(app.viewer_load.is_none());
        assert_eq!(app.viewer_load_phase, 0.0);
    }

    #[test]
    fn stale_generation_completion_does_not_display_the_old_scene() {
        use crate::inspector::scene3d::{BaseOrientation, Scene};

        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        app.begin_viewer_load((0, 0), "first.dff".to_string());
        // Entries mutated while the load was in flight: generation 0 -> 1.
        app.editor.archives_mut()[0].invalidate_entry_caches();

        let _ = app.update(Message::Viewer3dLoadCompleted {
            archive_index: 0,
            entry_index: 0,
            generation: 0,
            result: Ok(Arc::new(Scene::empty(BaseOrientation::Yup))),
            ide_map: None,
        });

        // The stale scene must not resolve onto the entry index's new
        // data: no viewer scene, no cache backfill, and the loading
        // transition for this target is released.
        assert!(app.active_viewer_entry.is_none());
        assert_eq!(app.scene_cache.len(), 0);
        assert!(app.viewer_load.is_none());
    }

    #[test]
    fn closing_an_archive_drops_its_in_flight_placeholder() {
        let mut app = test_app_with_entries();
        let archive = &app.editor.archives()[0];
        let key = (archive.file_name.clone(), archive.generation(), 0);
        let scene_cache = Arc::clone(&app.scene_cache);
        let guard = match scene_cache.get_value_or_guard(&key, Some(Duration::ZERO)) {
            GuardResult::Guard(guard) => guard,
            _ => panic!("fresh cache must yield a guard"),
        };
        app.begin_viewer_load((0, 0), "first.dff".to_string());

        let _ = app.update(Message::CloseSelectedArchive);

        // `retain` skips placeholders; the close must remove the key
        // explicitly so the late decode is discarded, not published.
        drop(guard);
        assert!(matches!(
            app.scene_cache.get_value_or_guard(&key, Some(Duration::ZERO)),
            GuardResult::Guard(_)
        ));
    }

    #[test]
    fn closing_archive_by_tab_index_repairs_selection() {
        let mut app = test_app();
        app.editor.new_archive();
        app.editor.new_archive();
        app.editor.select_archive(1);

        let _ = app.update(Message::CloseArchiveTab(0));

        assert_eq!(app.editor.archives().len(), 1);
        assert_eq!(app.editor.selected_archive(), Some(0));
    }

    #[test]
    fn placeholder_guard_publishes_atomically_and_blocks_duplicate_claims() {
        let cache: quick_cache::sync::Cache<u32, Arc<String>, quick_cache::UnitWeighter> =
            quick_cache::sync::Cache::new(4);
        match cache.get_value_or_guard(&1, Some(Duration::ZERO)) {
            GuardResult::Guard(guard) => {
                // While the guard is alive, a concurrent requester sees
                // the load in flight instead of claiming a second decode.
                assert!(matches!(
                    cache.get_value_or_guard(&1, Some(Duration::ZERO)),
                    GuardResult::Timeout
                ));
                guard
                    .insert(Arc::new("decoded".to_string()))
                    .expect("placeholder still in the cache");
            }
            _ => panic!("fresh cache must yield a guard"),
        }
        let published = cache.get(&1).expect("value must be published");
        assert_eq!(published.as_str(), "decoded");
    }

    #[test]
    fn scene_load_single_flights_duplicate_requests() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        let archive = &app.editor.archives()[0];
        let key = (archive.file_name.clone(), archive.generation(), 0);
        // Clone the Arc so the guard borrows the local handle, not `app`.
        let scene_cache = Arc::clone(&app.scene_cache);
        let guard = match scene_cache.get_value_or_guard(&key, Some(Duration::ZERO)) {
            GuardResult::Guard(guard) => guard,
            _ => panic!("fresh cache must yield a guard"),
        };

        let task = app.update(Message::Viewer3dRequestLoad {
            archive_index: 0,
            entry_index: 0,
        });
        assert!(
            matches!(drain_task(task).as_slice(), [Message::Noop]),
            "a load already in flight must not be duplicated"
        );
        drop(guard);
    }

    #[test]
    fn scene_load_failure_releases_the_placeholder_for_retry() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        let archive = &app.editor.archives()[0];
        let key = (archive.file_name.clone(), archive.generation(), 0);

        // The synthetic entry cannot be parsed: the task must report the
        // failure and release its placeholder so a retry can claim it.
        let task = app.update(Message::Viewer3dRequestLoad {
            archive_index: 0,
            entry_index: 0,
        });
        let messages = drain_task(task);
        assert!(matches!(
            messages.as_slice(),
            [Message::Viewer3dLoadCompleted {
                result: Err(_),
                ..
            }]
        ));
        assert!(app.scene_cache.get(&key).is_none());
        assert!(matches!(
            app.scene_cache.get_value_or_guard(&key, Some(Duration::ZERO)),
            GuardResult::Guard(_)
        ));
    }

    #[test]
    fn texture_decode_single_flights_duplicate_requests() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        let texture_cache = Arc::clone(&app.editor.archives()[0].texture_cache);
        let guard = match texture_cache.get_value_or_guard(&0, Some(Duration::ZERO)) {
            GuardResult::Guard(guard) => guard,
            _ => panic!("fresh cache must yield a guard"),
        };

        let messages = drain_task(app.decode_texture_entry(0));
        assert!(
            matches!(messages.as_slice(), [Message::Noop]),
            "a decode already in flight must not be duplicated"
        );
        drop(guard);
    }

    #[test]
    fn compatibility_validation_stores_report_and_toasts_summary() {
        use crate::compat::raster::Severity;
        use crate::compat::scan::ScanReport;

        let mut app = test_app_with_entries();
        let mut report = ScanReport {
            archive_path: "fixture.img".into(),
            archive_kind: "IMG v1".into(),
            entry_count: 3,
            target: Some("gta3"),
            ..ScanReport::default()
        };
        report.txd_entries = 2;
        report.textures = 5;
        report
            .class_counts
            .insert("DXT1".to_string(), 4);
        report
            .class_counts
            .insert("888 (32bpp storage)".to_string(), 1);
        report
            .verdicts
            .entry("gta3")
            .or_default()
            .insert("convertible (lossless)", 5);
        report
            .verdicts
            .entry("sa")
            .or_default()
            .insert("native", 4);
        report
            .verdicts
            .entry("sa")
            .or_default()
            .insert("supported", 1);
        report
            .anomaly_counts
            .insert("DIMS_NOT_POT", 2);
        report
            .anomaly_severity
            .insert("DIMS_NOT_POT", Severity::Error);

        let _ = app.update(Message::CompatibilityValidated {
            archive_index: 0,
            result: Ok(report),
        });

        let archive = &app.editor.archives()[0];
        assert!(archive.compat_report.is_some(), "report stored");
        let stored = archive.compat_report.as_ref().unwrap();
        assert_eq!(stored.textures, 5);
        assert_eq!(stored.error_count(), 2);
        let toast = app.toast.as_deref().unwrap_or_default();
        assert!(
            toast.contains("Validated 2 TXDs (5 textures)"),
            "toast should summarize: {toast}"
        );
        assert!(toast.contains("2 errors"), "toast should count errors: {toast}");
        assert!(
            toast.contains("for GTA III"),
            "toast names the target: {toast}"
        );
        assert!(
            app.toast_extended_duration,
            "the long summary needs the extended 6.5 s toast"
        );

        // Mutating entries invalidates the stored report.
        app.editor.archives_mut()[0].invalidate_entry_caches();
        assert!(app.editor.archives()[0].compat_report.is_none());
    }

    #[test]
    fn progress_tick_does_not_rearm_the_bar_from_zero() {
        // Regression: TickProgress compared the animated visual against
        // animator.get(), which reads 0.0 once the finished animation is
        // reaped — every tick re-armed a 0 -> current fill and the bar
        // glitched from empty to full repeatedly while a task ran.
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0].progress.start();
        app.editor.archives_mut()[0].progress.set_percentage(0.98);

        // A tick with no registered animation must not start one from
        // zero: the visual baseline is the current value when idle.
        // (Re-arms while an animation is running continue from its
        // interpolated value by construction of animate_from_current.)
        let _ = app.update(Message::TickProgress);
        assert!(
            !app.animator.is_running(crate::ui::app::ANIM_PROGRESS),
            "idle tick must not re-arm the progress animation"
        );

        app.editor.archives_mut()[0].progress.finish();
    }

    #[test]
    fn validate_button_without_archive_toasts_guidance() {
        let mut app = test_app();
        let _ = app.update(Message::OpenValidatorPopup);
        assert_eq!(
            app.toast.as_deref(),
            Some("Open an archive first to validate it.")
        );
        assert!(!app.validator_popup_open);
    }

    #[test]
    fn compat_highlight_toggle_flips_the_flag() {
        let mut app = test_app_with_entries();
        assert!(app.compat_highlight_enabled);
        let _ = app.update(Message::SetCompatHighlight(false));
        assert!(!app.compat_highlight_enabled);
        let _ = app.update(Message::SetCompatHighlight(true));
        assert!(app.compat_highlight_enabled);
    }

    #[test]
    fn tab_resize_drag_tracks_deltas_and_clamps() {
        let mut app = test_app_with_entries();
        let start = app.archive_tab_width;

        let _ = app.update(Message::TabResizeStarted);
        assert!(app.tab_resize_drag.is_some());
        // First movement only anchors the drag; it must not jump the width
        // to the absolute cursor position.
        let _ = app.update(Message::TabResizeMoved(900.0));
        assert_eq!(app.archive_tab_width, start);

        // Deltas widen and compress the tabs, clamped at the bounds.
        let _ = app.update(Message::TabResizeMoved(940.0));
        assert_eq!(app.archive_tab_width, start + 40.0);
        let _ = app.update(Message::TabResizeMoved(0.0));
        assert_eq!(
            app.archive_tab_width,
            crate::config::ARCHIVE_TAB_WIDTH_MIN,
            "compression stops at the minimum"
        );
        let _ = app.update(Message::TabResizeMoved(10_000.0));
        assert_eq!(
            app.archive_tab_width,
            crate::config::ARCHIVE_TAB_WIDTH_MAX,
            "widening stops at the maximum"
        );

        // Releasing persists the width and ends the drag.
        let _ = app.update(Message::TabResizeEnded);
        assert!(app.tab_resize_drag.is_none());
        assert_eq!(
            app.config.archive_tab_width,
            crate::config::ARCHIVE_TAB_WIDTH_MAX
        );
        // Stray moves after release must not resize anything.
        let _ = app.update(Message::TabResizeMoved(0.0));
        assert_eq!(
            app.archive_tab_width,
            crate::config::ARCHIVE_TAB_WIDTH_MAX
        );
    }

    /// A tiny planned conversion for dialog-state tests.
    fn tiny_plan(target: &'static crate::compat::games::GameProfile) -> CompactPlan {
        let mut rgba = Vec::new();
        for i in 0..4u32 * 4 {
            rgba.extend([i as u8, 0, 0, 255]);
        }
        let image = crate::compat::convert::SourceImage {
            width: 4,
            height: 4,
            rgba,
            has_alpha: false,
            format_label: "test".to_string(),
            encoded_bytes: 64,
        };
        let plan = crate::compat::convert::plan_import(
            &image,
            target,
            "txd.img",
            None,
            crate::compat::encode::EncodeOptions::default(),
        )
        .expect("plan");
        CompactPlan(Arc::new(plan))
    }

    #[test]
    fn replace_applied_sets_the_override_and_dirties() {
        let mut app = test_app_with_entries();
        let task = app.update(Message::ReplaceApplied {
            archive_index: 0,
            entry_index: 1,
            result: Ok(AppliedBytes(Arc::new(vec![1, 2, 3]))),
        });
        let archive = &app.editor.archives()[0];
        assert!(
            archive.entries[1].override_bytes.is_some(),
            "the converted bytes must become the entry override"
        );
        assert!(archive.dirty);
        assert_eq!(
            app.toast.as_deref(),
            Some("Texture replaced - save the archive to write it.")
        );
        let _ = drain_task(task);
    }

    #[test]
    fn new_txd_confirmed_adds_a_unique_entry() {
        let mut app = test_app_with_entries();
        let plan = tiny_plan(&crate::compat::games::GTA3);
        let state = |texture_name: &str| NewTxdState {
            archive_index: 0,
            source_path: PathBuf::from("x.png"),
            source_name: "x.png".to_string(),
            texture_name: texture_name.to_string(),
            target: &crate::compat::games::GTA3,
            chooser: plan.0.format,
            high_quality: false,
            plan: plan.clone(),
            after_handle: iced::widget::image::Handle::from_rgba(4, 4, vec![0, 0, 0, 0]),
            planning: false,
        };

        app.pending_new_txd = Some(state("MyTex"));
        let _ = app.update(Message::NewTxdConfirmed);
        assert!(
            app.editor.archives()[0]
                .entries
                .iter()
                .any(|entry| entry.file_name == "MyTex.txd"
                    && entry.override_bytes.is_some()),
            "the new TXD entry must carry its bytes as an override"
        );
        assert!(app.editor.archives()[0].dirty);

        let before = app.editor.archives()[0].entries.len();
        app.pending_new_txd = Some(state("MyTex"));
        let _ = app.update(Message::NewTxdConfirmed);
        assert_eq!(
            app.editor.archives()[0].entries.len(),
            before,
            "duplicate names must be rejected"
        );
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|toast| toast.contains("already exists")),
            "{:?}",
            app.toast
        );
        assert!(
            app.pending_new_txd.is_some(),
            "a rejected name must keep the dialog open for a fix"
        );
    }

    #[test]
    fn converter_actions_without_a_usable_target_open_the_validator() {
        let mut app = test_app_with_entries();

        // No target: the import action explains and opens the picker.
        let _ = app.update(Message::ImportImageAsTxdRequested);
        assert!(app.validator_popup_open);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|toast| toast.contains("No target")),
            "{:?}",
            app.toast
        );

        // A stale/wrong target (Bully: no RenderWare writing) must not
        // dead-end either.
        app.validator_popup_open = false;
        app.editor.archives_mut()[0].target_game = Some("bully");
        let _ = app.update(Message::ImportImageAsTxdRequested);
        assert!(app.validator_popup_open);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|toast| toast.contains("Bully") && toast.contains("Validate")),
            "{:?}",
            app.toast
        );

        // The replace action routes the same way once a TXD is selected.
        app.validator_popup_open = false;
        app.editor.archives_mut()[0].target_game = None;
        app.editor.select_entry(1, false, false);
        let _ = app.update(Message::TextureReplaceRequested);
        assert!(app.validator_popup_open);
    }

    #[test]
    fn duplicate_converter_requests_are_ignored_while_planning() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0].target_game = Some("sa");

        // Import: a second request while a plan runs would bump the
        // attempt and silently drop the running plan.
        app.new_txd_plan_in_flight = true;
        let attempt = app.new_txd_attempt;
        let _ = app.update(Message::ImportImageAsTxdRequested);
        assert_eq!(app.new_txd_attempt, attempt, "no new attempt may start");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|toast| toast.contains("already being prepared")),
            "{:?}",
            app.toast
        );

        // Replace: same guard.
        app.editor.select_entry(1, false, false);
        app.replace_plan_in_flight = true;
        let attempt = app.replace_attempt;
        let _ = app.update(Message::TextureReplaceRequested);
        assert_eq!(app.replace_attempt, attempt);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|toast| toast.contains("already being prepared")),
            "{:?}",
            app.toast
        );
    }

    #[test]
    fn plan_completion_releases_the_in_flight_guard() {
        let mut app = test_app_with_entries();

        app.new_txd_plan_in_flight = true;
        app.new_txd_attempt = 2;
        let _ = app.update(Message::NewTxdPlanned {
            attempt: 2,
            result: Box::new(Err("nope".to_string())),
        });
        assert!(!app.new_txd_plan_in_flight);

        app.replace_plan_in_flight = true;
        app.replace_attempt = 2;
        let _ = app.update(Message::ReplacePlanned {
            attempt: 2,
            result: Box::new(Err("nope".to_string())),
        });
        assert!(!app.replace_plan_in_flight);
    }

    #[test]
    fn stale_new_txd_plans_never_reopen_the_dialog() {
        let mut app = test_app_with_entries();
        let ready = || NewTxdPlanReady {
            archive_index: 0,
            source_path: PathBuf::from("x.png"),
            source_name: "x.png".to_string(),
            texture_name: "x".to_string(),
            target: &crate::compat::games::GTA3,
            plan: tiny_plan(&crate::compat::games::GTA3),
        };

        // After a confirm (or cancel) the attempt was bumped and no
        // dialog is open: a completion from the superseded attempt must
        // be dropped instead of re-instating it.
        app.new_txd_attempt = 7;
        assert!(app.pending_new_txd.is_none());
        let _ = app.update(Message::NewTxdPlanned {
            attempt: 6,
            result: Box::new(Ok(ready())),
        });
        assert!(
            app.pending_new_txd.is_none(),
            "a stale plan must not re-open the dialog"
        );

        // The live attempt still opens it.
        let _ = app.update(Message::NewTxdPlanned {
            attempt: 7,
            result: Box::new(Ok(ready())),
        });
        assert!(app.pending_new_txd.is_some());
    }

    #[test]
    fn cancelling_new_txd_invalidates_in_flight_plans() {
        let mut app = test_app_with_entries();
        app.new_txd_attempt = 3;
        app.pending_new_txd = Some(NewTxdState {
            archive_index: 0,
            source_path: PathBuf::from("x.png"),
            source_name: "x.png".to_string(),
            texture_name: "x".to_string(),
            target: &crate::compat::games::GTA3,
            chooser: crate::compat::encode::EncodeFormat::Rgb888,
            high_quality: false,
            plan: tiny_plan(&crate::compat::games::GTA3),
            after_handle: iced::widget::image::Handle::from_rgba(4, 4, vec![0, 0, 0, 0]),
            planning: true,
        });

        let _ = app.update(Message::NewTxdCancelled);
        assert!(app.pending_new_txd.is_none());
        assert_eq!(app.new_txd_attempt, 4);

        let ready = NewTxdPlanReady {
            archive_index: 0,
            source_path: PathBuf::from("x.png"),
            source_name: "x.png".to_string(),
            texture_name: "x".to_string(),
            target: &crate::compat::games::GTA3,
            plan: tiny_plan(&crate::compat::games::GTA3),
        };
        let _ = app.update(Message::NewTxdPlanned {
            attempt: 3,
            result: Box::new(Ok(ready)),
        });
        assert!(
            app.pending_new_txd.is_none(),
            "cancel must invalidate plans still in flight"
        );
    }

    #[test]
    fn stale_replace_refresh_does_not_reopen_after_cancel() {
        let mut app = test_app_with_entries();
        app.replace_attempt = 4;
        assert!(app.pending_replace.is_none());
        let _ = app.update(Message::ReplacePlanRefreshed {
            attempt: 3,
            result: Box::new(Err("stale".to_string())),
        });
        assert!(app.pending_replace.is_none());
        assert!(
            app.toast.is_none(),
            "a superseded failure must not toast either: {:?}",
            app.toast
        );
    }

    #[test]
    fn cancelled_pickers_release_their_flow_state() {
        let mut app = test_app_with_entries();

        // Cancelling the replace picker must not leave the request
        // armed, or every later replacement would be refused.
        app.replace_attempt = 1;
        app.replace_request = Some((0, 1));
        let _ = app.update(Message::ReplaceImagePicked {
            attempt: 1,
            path: None,
        });
        assert!(app.replace_request.is_none());

        // Cancelling the new-TXD picker releases the double-open guard.
        app.new_txd_attempt = 1;
        app.new_txd_picker_open = true;
        let _ = app.update(Message::NewTxdImagePicked {
            attempt: 1,
            path: None,
        });
        assert!(!app.new_txd_picker_open);
    }

    #[test]
    fn bulk_planned_opens_the_dialog_and_cancel_clears_it() {
        let mut app = test_app_with_entries();
        let ready = BulkPlanReady {
            archive_index: 0,
            source_label: "gta3.img".to_string(),
            entries: vec![BulkEntryPlan {
                entry_index: 1,
                file_name: "second.txd".to_string(),
                textures: vec![(0, "tex".to_string(), tiny_plan(&crate::compat::games::GTA3))],
                skipped_native: 2,
                failed: 0,
            }],
        };
        let _ = app.update(Message::BulkConvertPlanned(Box::new(Ok(ready))));
        assert!(app.pending_bulk.is_some(), "the dialog must open");
        let _ = app.update(Message::BulkConvertCancelled);
        assert!(app.pending_bulk.is_none());
    }

    #[test]
    fn bulk_convert_applied_sets_overrides_and_dirties() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::BulkConvertApplied {
            archive_index: 0,
            result: Ok(SavePatches(vec![(1, Arc::new(vec![7, 7, 7]))])),
        });
        assert!(app.editor.archives()[0].entries[1].override_bytes.is_some());
        assert!(app.editor.archives()[0].dirty);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|toast| toast.contains("Converted 1 entries")),
            "{:?}",
            app.toast
        );
    }

    #[test]
    fn save_gates_on_issues_and_confirm_clears_the_dialog() {
        use crate::compat::scan::ScanReport;

        let mut app = test_app_with_entries();
        {
            let archive = &mut app.editor.archives_mut()[0];
            archive.target_game = Some("gta3");
            let mut report = ScanReport::default();
            report.verdicts.entry("gta3").or_default().insert("unsupported", 3);
            report.textures = 3;
            archive.compat_report = Some(report);
        }

        // Issues present: the save pauses on the report dialog.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.img");
        let _ = app.begin_save(
            app.editor.archives()[0].clone(),
            path.clone(),
            crate::parser::ImgVersion::One,
            true,
        );
        assert!(app.pending_save.is_some(), "issues must gate the save");
        assert!(app.modal_open(), "the report gates shortcuts");

        // Confirming writes (the task completes with an outcome) and
        // clears the state.
        let messages = drain_task(app.update(Message::SaveCheckConfirmed));
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::SaveCompleted { .. })),
            "confirming must start the save: {messages:?}"
        );
        assert!(app.pending_save.is_none());
        assert!(
            !app.editor.archives()[0].progress.in_use(),
            "a finished save must release the progress slot (else the shield refuses later scans)"
        );

        // Cancelling keeps the archive untouched.
        app.pending_save = Some(PendingSave {
            index: 0,
            path,
            version: crate::parser::ImgVersion::One,
            remove_existing: true,
            issue: Some(crate::compat::save::SaveIssue::default()),
            fix: false,
        });
        let _ = app.update(Message::SaveCheckCancelled);
        assert!(app.pending_save.is_none());
        assert_eq!(app.toast.as_deref(), Some("Save cancelled."));
    }

    #[test]
    fn save_check_repair_toggle_plans_in_background_then_applies() {
        use crate::compat::raster::Severity;
        use crate::compat::scan::ScanReport;

        let mut app = test_app_with_entries();
        {
            let archive = &mut app.editor.archives_mut()[0];
            archive.target_game = Some("gta3");
            let mut report = ScanReport {
                textures: 1,
                ..ScanReport::default()
            };
            report.anomaly_counts.insert("CONTRADICTORY_DXT_HEADER", 1);
            report
                .anomaly_severity
                .insert("CONTRADICTORY_DXT_HEADER", Severity::Error);
            archive.compat_report = Some(report);
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.img");
        let _ = app.begin_save(
            app.editor.archives()[0].clone(),
            path.clone(),
            crate::parser::ImgVersion::One,
            true,
        );
        assert!(app.pending_save.is_some(), "a broken header must gate");
        assert!(
            app.pending_save.as_ref().unwrap().fix,
            "the repair default follows the fixable reports"
        );

        let _ = app.update(Message::SaveCheckFixToggled(false));
        assert!(!app.pending_save.as_ref().unwrap().fix);
        let _ = app.update(Message::SaveCheckFixToggled(true));
        assert!(app.pending_save.as_ref().unwrap().fix);

        // Confirming with repair returns a planning task first.
        let messages = drain_task(app.update(Message::SaveCheckConfirmed));
        let ready = messages
            .iter()
            .find(|message| matches!(message, Message::SaveFixesReady { .. }))
            .cloned()
            .expect("confirming with repair must plan before saving");
        assert!(app.pending_save.is_none());

        // Feeding the plan back starts the save; a synthetic entry has no
        // readable bytes, so the plan is empty and nothing is patched.
        let messages = drain_task(app.update(ready));
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::SaveCompleted { .. })),
            "the save must run after planning: {messages:?}"
        );

        // A non-empty plan applies the overrides and dirties the archive.
        let patch = SavePatches(vec![(1, Arc::new(vec![9, 9, 9, 9]))]);
        let task = app.update(Message::SaveFixesReady {
            index: 0,
            patches: patch,
            path,
            version: crate::parser::ImgVersion::One,
            remove_existing: true,
        });
        assert!(app.editor.archives()[0].entries[1].override_bytes.is_some());
        assert!(app.editor.archives()[0].dirty, "a patch changes the archive");
        let messages = drain_task(task);
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::SaveCompleted { .. })),
            "the patched archive must still save: {messages:?}"
        );
    }

    #[test]
    fn save_with_a_clean_report_skips_the_dialog() {
        use crate::compat::scan::ScanReport;

        let mut app = test_app_with_entries();
        {
            let archive = &mut app.editor.archives_mut()[0];
            archive.target_game = Some("gta3");
            let mut report = ScanReport::default();
            report.verdicts.entry("gta3").or_default().insert("native", 10);
            report.textures = 10;
            archive.compat_report = Some(report);
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.img");
        let task = app.begin_save(
            app.editor.archives()[0].clone(),
            path,
            crate::parser::ImgVersion::One,
            true,
        );
        assert!(
            app.pending_save.is_none(),
            "a clean save must not interrupt the user"
        );
        // The save still runs; the synthetic archive fails to write, but
        // the outcome must come back without any dialog.
        let messages = drain_task(task);
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::SaveCompleted { .. })),
            "clean save must proceed: {messages:?}"
        );
        assert!(
            !app.editor.archives()[0].progress.in_use(),
            "the progress slot must be released after a save"
        );
    }

    #[test]
    fn closing_a_dirty_archive_asks_before_discarding() {
        let mut app = test_app_with_entries();

        // Dirty: guarded.
        app.editor.archives_mut()[0].dirty = true;
        let _ = app.update(Message::CloseArchiveTab(0));
        assert!(matches!(
            app.pending_close,
            Some(PendingClose::Archive(0))
        ));
        assert!(app.modal_open());
        assert_eq!(app.editor.archives().len(), 1, "still open while asking");

        // Cancel keeps it open.
        let _ = app.update(Message::CloseGuardCancel);
        assert!(app.pending_close.is_none());
        assert_eq!(app.editor.archives().len(), 1);

        // Discard closes it.
        let _ = app.update(Message::CloseArchiveTab(0));
        let _ = app.update(Message::CloseGuardDiscard);
        assert!(app.pending_close.is_none());
        assert!(app.editor.archives().is_empty());

        // Clean tabs close without asking.
        let mut app = test_app_with_entries();
        let _ = app.update(Message::CloseSelectedArchive);
        assert!(app.pending_close.is_none());
        assert!(app.editor.archives().is_empty());
    }

    #[test]
    fn quitting_with_dirty_archives_asks_before_closing_the_window() {
        let mut app = test_app_with_entries();
        let window = iced::window::Id::unique();

        // Clean: the close request passes straight through.
        let _ = app.update(Message::WindowCloseRequested(window));
        assert!(app.pending_close.is_none());

        // Dirty: guarded, and cancelling keeps the window.
        app.editor.archives_mut()[0].dirty = true;
        let _ = app.update(Message::WindowCloseRequested(window));
        assert!(matches!(
            app.pending_close,
            Some(PendingClose::Window(_))
        ));
        let _ = app.update(Message::CloseGuardCancel);
        assert!(app.pending_close.is_none());
        assert_eq!(app.editor.archives().len(), 1);
    }

    #[test]
    fn guard_save_closes_the_archive_after_a_successful_save() {
        use crate::compat::scan::ScanReport;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard.img");
        let mut img: Vec<u8> = Vec::new();
        img.extend_from_slice(b"VER2");
        img.extend_from_slice(&0_u32.to_le_bytes());
        img.resize(2048, 0);
        std::fs::write(&path, &img).unwrap();

        let mut app = test_app();
        let _ = app.editor.add_opened_archive(ArchiveInfo::open(&path).unwrap());
        {
            let archive = &mut app.editor.archives_mut()[0];
            archive.dirty = true;
            archive.target_game = Some("sa");
            let mut report = ScanReport::default();
            report.verdicts.entry("sa").or_default().insert("native", 1);
            report.textures = 1;
            archive.compat_report = Some(report);
        }

        let _ = app.update(Message::CloseArchiveTab(0));
        let task = app.update(Message::CloseGuardSave);
        assert_eq!(app.close_after_save, Some(0));
        let messages = drain_task(task);
        let saved = messages
            .iter()
            .find(|message| matches!(message, Message::SaveCompleted { .. }))
            .cloned()
            .expect("save must complete");
        // `Save` on a clean report saves silently and then closes.
        let _ = app.update(saved);
        assert!(app.close_after_save.is_none());
        assert!(
            app.editor.archives().is_empty(),
            "the archive must close after the guard's save"
        );
    }

    #[test]
    fn confirmed_quit_fades_out_before_closing_the_window() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0].dirty = true;
        let window = iced::window::Id::unique();

        let _ = app.update(Message::WindowCloseRequested(window));
        assert!(matches!(app.pending_close, Some(PendingClose::Window(_))));

        // Discard does not close immediately: it starts the fade and
        // schedules the deadline that finally closes the window.
        let task = app.update(Message::CloseGuardDiscard);
        assert!(app.pending_close.is_none());
        assert_eq!(app.quitting, Some(window), "fade started");
        assert!(
            app.animator.is_running(crate::ui::app::ANIM_QUIT_FADE),
            "the fade animation must drive the overlay"
        );
        let messages = drain_task(task);
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::QuitFadeDone)),
            "the close deadline must fire: {messages:?}"
        );
        // The fade deadline only starts the native hide. The quit state must
        // remain active until that hide has had time to reach the window.
        let hide_task = app.update(Message::QuitFadeDone);
        assert_eq!(app.quitting, Some(window), "fade remains active while hiding");
        let hidden_messages = drain_task(hide_task);
        let hidden = hidden_messages
            .into_iter()
            .find(|message| matches!(message, Message::QuitWindowHidden))
            .expect("the hide settle deadline must fire");
        let _ = app.update(hidden);
        assert!(app.quitting.is_none(), "the window is closed once hidden");
    }

    #[test]
    fn escape_dismisses_pending_dialogs() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0].dirty = true;

        // Close guard: Escape cancels the close request.
        let _ = app.update(Message::CloseArchiveTab(0));
        assert!(app.pending_close.is_some());
        let _ = app.update(Message::ClearSelection);
        assert!(app.pending_close.is_none());
        assert_eq!(app.editor.archives().len(), 1, "archive stays open");

        // Pre-save report: Escape cancels the save and any guard chain.
        app.close_after_save = Some(0);
        app.pending_save = Some(PendingSave {
            index: 0,
            path: std::path::PathBuf::from("x.img"),
            version: crate::parser::ImgVersion::One,
            remove_existing: false,
            issue: Some(crate::compat::save::SaveIssue::default()),
            fix: false,
        });
        let _ = app.update(Message::ClearSelection);
        assert!(app.pending_save.is_none());
        assert!(app.close_after_save.is_none());
    }

    #[test]
    fn validator_popup_opens_closes_and_runs_for_a_target() {
        let mut app = test_app_with_entries();
        assert!(!app.modal_open());

        let _ = app.update(Message::OpenValidatorPopup);
        assert!(app.validator_popup_open);
        assert!(app.modal_open(), "keyboard shortcuts must be gated");

        let _ = app.update(Message::CloseValidatorPopup);
        assert!(!app.validator_popup_open);

        // Running closes the picker (the worker drives the progress bar).
        // The synthetic test archive has no source path, so the scan
        // errors out - but the popup must still close and the progress
        // slot must be released (regression: start() before the path
        // check left it stuck in use).
        let _ = app.update(Message::OpenValidatorPopup);
        assert!(app.validator_popup_open);
        let messages = drain_task(app.update(Message::ValidateArchiveFor("sa")));
        assert!(
            matches!(
                messages.as_slice(),
                [Message::CompatibilityValidated {
                    archive_index: 0,
                    ..
                }]
            ),
            "validation must report back: {messages:?}"
        );
        assert!(
            !app.validator_popup_open,
            "running closes the picker so the highlighted rows are visible"
        );
        assert!(
            !app.editor.archives()[0].progress.in_use(),
            "a failed scan must release the progress slot"
        );
    }

    #[test]
    fn import_preflight_flags_incompatible_files_and_opens_the_dialog() {
        let dir = tempfile::tempdir().unwrap();
        // A Gamebryo NFT with a DXT1 raster: harmless for Bully,
        // incompatible for a GTA III target.
        let nft = dir.path().join("skin.nft");
        std::fs::write(
            &nft,
            crate::inspector::nif::tests::build_nif(&[(
                "NiPixelData",
                &{
                    let mut block = Vec::new();
                    block.extend_from_slice(&4_u32.to_le_bytes());
                    block.push(0);
                    block.extend_from_slice(&(-1_i32).to_le_bytes());
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    block.push(1);
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    block.push(0);
                    block.extend_from_slice(&[4, 0, 0, 0]);
                    block.extend_from_slice(&(-1_i32).to_le_bytes());
                    block.extend_from_slice(&1_u32.to_le_bytes());
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    block.extend_from_slice(&8_u32.to_le_bytes());
                    block.extend_from_slice(&8_u32.to_le_bytes());
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    block.extend_from_slice(&32_u32.to_le_bytes());
                    block.extend_from_slice(&1_u32.to_le_bytes());
                    block.extend(std::iter::repeat_n(0x8A_u8, 32));
                    block
                },
            )]),
        )
        .unwrap();

        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0].target_game = Some("gta3");

        // Not blocked while the check runs.
        let task = app.begin_import(0, app.editor.archives()[0].clone(), vec![nft.clone()], None);
        let messages = drain_task(task);
        assert!(matches!(
            messages.as_slice(),
            [Message::ImportPreflightCompleted { .. }]
        ));
        for message in messages {
            let _ = app.update(message);
        }
        assert!(app.pending_import.is_some(), "flagged import must pause");
        assert!(app.modal_open(), "the dialog must gate shortcuts");
        assert_eq!(app.pending_import.as_ref().unwrap().flagged().len(), 1);

        // Cancelling drops the pending import.
        let _ = app.update(Message::ImportCheckCancelled);
        assert!(app.pending_import.is_none());
        assert_eq!(app.toast.as_deref(), Some("Import cancelled."));
    }

    #[test]
    fn import_without_a_target_skips_the_preflight() {
        let dir = tempfile::tempdir().unwrap();
        let nft = dir.path().join("skin.nft");
        std::fs::write(&nft, b"Gamebryo File Format, Version 20.3.0.9\n").unwrap();

        let mut app = test_app_with_entries();
        assert!(app.editor.archives()[0].target_game.is_none());
        let task = app.begin_import(0, app.editor.archives()[0].clone(), vec![nft], None);
        // No pre-flight task is scheduled: the import goes straight in.
        let messages = drain_task(task);
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::ImportCompleted { checked: false, .. })),
            "unchecked import must report itself as unchecked: {messages:?}"
        );
    }

    #[test]
    fn imported_entries_get_their_verdicts_merged_into_the_kept_report() {
        let dir = tempfile::tempdir().unwrap();
        let nft = dir.path().join("skin.nft");
        std::fs::write(
            &nft,
            crate::inspector::nif::tests::build_nif(&[(
                "NiPixelData",
                &{
                    let mut block = Vec::new();
                    block.extend_from_slice(&4_u32.to_le_bytes()); // DXT1
                    block.push(0);
                    block.extend_from_slice(&(-1_i32).to_le_bytes());
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    block.push(1);
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    block.push(0);
                    block.extend_from_slice(&[4, 0, 0, 0]);
                    block.extend_from_slice(&(-1_i32).to_le_bytes());
                    block.extend_from_slice(&1_u32.to_le_bytes());
                    block.extend_from_slice(&0_u32.to_le_bytes());
                    for value in [8_u32, 8, 0, 32, 1] {
                        block.extend_from_slice(&value.to_le_bytes());
                    }
                    block.extend(std::iter::repeat_n(0x8A_u8, 32));
                    block
                },
            )]),
        )
        .unwrap();

        let mut app = test_app_with_entries();
        {
            let archive = &mut app.editor.archives_mut()[0];
            archive.target_game = Some("bully");
            // A report from a previous validation (row verdicts stay).
            archive.compat_report = Some(crate::compat::scan::ScanReport::default());
            // An imported entry backed by the NFT on disk.
            let mut entry = EntryInfo::new("skin.nft");
            entry.imported = true;
            entry.source_path = Some(nft);
            archive.entries.push(entry);
            archive.invalidate_entry_caches_keeping_report();
        }

        assert!(app.editor.archives()[0].compat_report.is_some());
        app.refresh_imported_verdicts(0);

        let report = app.editor.archives()[0].compat_report.as_ref().unwrap();
        let merged = report
            .entry_verdicts
            .iter()
            .find(|verdict| verdict.file_name == "skin.nft")
            .expect("imported entry must gain a verdict");
        assert_eq!(merged.textures, 1);
        assert_eq!(merged.worst, crate::compat::games::Verdict::Native);
    }

    #[test]
    fn picking_a_target_persists_it_and_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gta3.img");
        // A minimal VER2 archive so the real open path is exercised.
        let mut img: Vec<u8> = Vec::new();
        img.extend_from_slice(b"VER2");
        img.extend_from_slice(&0_u32.to_le_bytes());
        img.resize(2048, 0);
        std::fs::write(&path, &img).unwrap();

        let mut app = test_app();
        let archive = ArchiveInfo::open(&path).unwrap();
        let _ = app.editor.add_opened_archive(archive);

        let _ = app.update(Message::ValidateArchiveFor("sa"));
        assert_eq!(app.editor.archives()[0].target_game, Some("sa"));
        assert_eq!(app.config.archive_target(&path), Some("sa"));

        // A fresh session (same config) re-applies the saved target.
        let reopened = ArchiveInfo::open(&path).unwrap();
        let mut fresh = App::new(app.config.clone());
        fresh.config_persist_enabled = false;
        let _ = fresh.editor.add_opened_archive(reopened);
        if let Some(archive) = fresh.editor.archives_mut().last_mut() {
            App::adopt_target(&fresh.config, archive);
        }
        assert_eq!(fresh.editor.archives()[0].target_game, Some("sa"));
    }

    #[test]
    fn texture_decode_failure_releases_the_placeholder_for_retry() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(0, false, false);
        let texture_cache = Arc::clone(&app.editor.archives()[0].texture_cache);

        // The synthetic entry is not a TXD/NFT container: the decode fails,
        // which must release the placeholder instead of wedging the entry.
        let messages = drain_task(app.decode_texture_entry(0));
        assert!(matches!(
            messages.as_slice(),
            [Message::TextureDecoded {
                result: Err(_),
                ..
            }]
        ));
        assert!(texture_cache.get(&0).is_none());
        assert!(matches!(
            texture_cache.get_value_or_guard(&0, Some(Duration::ZERO)),
            GuardResult::Guard(_)
        ));
    }

    #[test]
    fn clicking_entry_starts_feedback_at_last_pointer_position() {
        let mut app = test_app_with_entries();
        let pointer = Point::new(120.0, 240.0);
        let _ = app.update(Message::PointerMoved(pointer));

        let _ = app.update(Message::EntryClicked(0));

        assert_eq!(app.editor.selected_entry(), Some(0));
        assert!(app.animator.is_running(ANIM_ENTRY_FEEDBACK));
        assert!(app.animator.is_running(ANIM_CLICK_RIPPLE));
        let visual = app
            .ripple_visual(RippleTarget::Entry {
                archive_index: 0,
                entry_index: 0,
            })
            .expect("entry click should create a ripple");
        assert_eq!(visual.origin, Some(pointer));
    }

    #[test]
    fn clicking_another_entry_after_idle_restarts_visible_feedback() {
        let mut app = test_app_with_entries();
        let first_tick = std::time::Instant::now();

        let _ = app.update(Message::EntryClicked(0));
        let _ = app.update(Message::AnimationTick(first_tick));
        for step in 1..=7 {
            let _ = app.update(Message::AnimationTick(
                first_tick + Duration::from_millis(50 * step),
            ));
        }
        assert!(!app.animator.is_running(ANIM_ENTRY_FEEDBACK));
        assert!(!app.animator.is_running(ANIM_CLICK_RIPPLE));

        let _ = app.update(Message::EntryClicked(1));
        let second_tick = first_tick + Duration::from_secs(5);
        let _ = app.update(Message::AnimationTick(second_tick));
        let _ = app.update(Message::AnimationTick(
            second_tick + Duration::from_millis(50),
        ));

        assert!(app.animator.is_running(ANIM_ENTRY_FEEDBACK));
        assert!(app.animator.is_running(ANIM_CLICK_RIPPLE));
        assert!(app.entry_selection_pulse((0, 1)) > 0.0);
        assert!(
            app.ripple_visual(RippleTarget::Entry {
                archive_index: 0,
                entry_index: 1,
            })
            .is_some()
        );
    }

    #[test]
    fn disabling_motion_prevents_new_interaction_tracks() {
        let mut app = test_app_with_entries();
        app.config.motion_enabled = false;

        let _ = app.update(Message::EntryClicked(0));

        assert!(!app.animator.is_running(ANIM_ENTRY_FEEDBACK));
        assert!(!app.animator.is_running(ANIM_CLICK_RIPPLE));
        assert!(
            app.ripple_visual(RippleTarget::Entry {
                archive_index: 0,
                entry_index: 0,
            })
            .is_none()
        );
    }

    #[test]
    fn clear_selection_shortcut_clears_selection_and_preview_target() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(1, false, false);
        app.active_viewer_entry = Some((0, 1));
        app.selected_texture = 3;
        app.show_texture_uv = true;

        let _ = app.handle_shortcut(Shortcut::ClearSelection);
        let _ = app.update(Message::ClearSelection);

        assert_eq!(app.editor.selected_entry(), None);
        assert!(
            app.editor.archives()[0]
                .entries
                .iter()
                .all(|entry| !entry.selected)
        );
        assert_eq!(app.active_viewer_entry, None);
        assert_eq!(app.selected_texture, 0);
        assert!(!app.show_texture_uv);
    }

    #[test]
    fn shortcuts_are_ignored_while_text_input_is_focused() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0].entries[0].selected = true;

        let _ = app.update(Message::ShortcutPressed(Shortcut::Delete));
        let _ = app.update(Message::SearchFocusChanged(true));
        let _ = app.update(Message::RenameFocusChanged(false));
        assert_eq!(app.editor.archives()[0].entries.len(), 2);

        let _ = app.update(Message::ShortcutPressed(Shortcut::Delete));
        let _ = app.update(Message::SearchFocusChanged(false));
        let _ = app.update(Message::RenameFocusChanged(false));
        let _ = app.update(Message::DeleteSelected);
        assert_eq!(app.editor.archives()[0].entries.len(), 1);
    }

    #[test]
    fn focus_search_shortcut_marks_search_focused() {
        let mut app = test_app();
        let _ = app.handle_shortcut(Shortcut::FocusSearch);
        assert!(app.search_focused);
    }

    #[test]
    fn shortcut_pipeline_switches_inspector_tab_when_no_input_focused() {
        // End-to-end check for the digit chords, including the focus
        // handshake: no text inputs exist in this state, which used to
        // deadlock the check and swallow every shortcut.
        let mut app = test_app();
        assert_eq!(app.selected_inspector_tab, InspectorTab::Export);

        let _ = app.update(Message::ShortcutPressed(Shortcut::SwitchTab(
            InspectorTab::Model3D,
        )));
        let _ = app.update(Message::SearchFocusChanged(false));
        let follow_up = drain_task(app.update(Message::RenameFocusChanged(false)));
        assert_eq!(follow_up.len(), 1);
        assert!(matches!(
            follow_up[0],
            Message::Viewer3dSelectTab(InspectorTab::Model3D)
        ));
        let _ = app.update(Message::Viewer3dSelectTab(InspectorTab::Model3D));
        assert_eq!(app.selected_inspector_tab, InspectorTab::Model3D);

        let _ = app.update(Message::ShortcutPressed(Shortcut::SwitchTab(
            InspectorTab::Texture,
        )));
        let _ = app.update(Message::SearchFocusChanged(false));
        let follow_up = drain_task(app.update(Message::RenameFocusChanged(false)));
        assert_eq!(follow_up.len(), 1);
        assert!(matches!(
            follow_up[0],
            Message::Viewer3dSelectTab(InspectorTab::Texture)
        ));
        let _ = app.update(Message::Viewer3dSelectTab(InspectorTab::Texture));
        assert_eq!(app.selected_inspector_tab, InspectorTab::Texture);
    }

    #[test]
    fn ctrl_d_deselects_entries() {
        let mut app = test_app_with_entries();
        app.editor.select_entry(1, false, false);
        assert!(app.editor.selected_entry().is_some());

        let _ = app.update(Message::ShortcutPressed(Shortcut::ClearSelection));
        let _ = app.update(Message::SearchFocusChanged(false));
        let follow_up = drain_task(app.update(Message::RenameFocusChanged(false)));
        assert_eq!(follow_up.len(), 1);
        assert!(matches!(follow_up[0], Message::ClearSelection));
        let _ = app.update(Message::ClearSelection);
        assert_eq!(app.editor.selected_entry(), None);
    }

    #[test]
    fn toggling_literal_file_types_updates_mode_and_cache() {
        let mut app = test_app_with_entries();
        // Warm the curated type cache first.
        let _ = app.update(Message::DebounceTick);

        let _ = app.update(Message::ToggleLiteralFileTypes(true));
        assert!(app.config.literal_file_types);
        assert!(app.editor.file_type_literal);
        // The per-archive cache was invalidated: the next read sees
        // literal extensions, not the stale curated labels.
        let types = app
            .editor
            .archives_mut()[0]
            .unique_file_types(true)
            .to_vec();
        assert_eq!(types, ["DFF", "TXD"].as_slice());

        let _ = app.update(Message::ToggleLiteralFileTypes(false));
        assert!(!app.config.literal_file_types);
        assert!(!app.editor.file_type_literal);
    }

    #[test]
    fn toggling_context_accumulate_switches_right_click_selection_mode() {
        let mut app = test_app_with_entries();
        assert!(app.config.context_selection_accumulates);
        assert!(app.editor.context_selection_accumulates);

        let _ = app.update(Message::ToggleContextAccumulate(false));
        assert!(!app.config.context_selection_accumulates);
        assert!(!app.editor.context_selection_accumulates);

        // Right-clicking a second entry replaces the selection.
        app.editor.archives_mut()[0].entries[0].selected = true;
        let _ = app.update(Message::EntryRightClicked(1));
        let selected: Vec<bool> = app.editor.archives()[0]
            .entries
            .iter()
            .map(|e| e.selected)
            .collect();
        assert_eq!(selected, vec![false, true]);
        assert!(app.context_menu.is_some());

        // Right-clicking an already-selected member of a multi-selection
        // (select-all + right-click) keeps the group intact.
        let _ = app.update(Message::SelectAll);
        let _ = app.update(Message::EntryRightClicked(0));
        let selected: Vec<bool> = app.editor.archives()[0]
            .entries
            .iter()
            .map(|e| e.selected)
            .collect();
        assert_eq!(selected, vec![true, true]);

        let _ = app.update(Message::ToggleContextAccumulate(true));
        let _ = app.update(Message::EntryRightClicked(0));
        let selected: Vec<bool> = app.editor.archives()[0]
            .entries
            .iter()
            .map(|e| e.selected)
            .collect();
        assert_eq!(selected, vec![true, true]);
    }

    #[test]
    fn clear_search_resets_query_and_refocuses() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::SearchChanged("first".to_string()));
        let _ = app.update(Message::DebounceTick);
        assert_eq!(app.editor.archives()[0].selected_indices.len(), 1);

        let _ = app.update(Message::ClearSearch);
        assert!(app.search.is_empty());
        assert!(app.search_focused);
        let _ = app.update(Message::DebounceTick);
        assert_eq!(
            app.editor.archives()[0].selected_indices.len(),
            2,
            "clearing must reveal every entry again"
        );
    }

    #[test]
    fn hiding_search_bar_clears_the_active_filter() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::SearchChanged("first".to_string()));
        let _ = app.update(Message::DebounceTick);
        assert_eq!(
            app.editor.archives()[0].selected_indices.len(),
            1,
            "filter should match exactly one entry"
        );

        let _ = app.update(Message::ToggleSearchBar(false));
        let _ = app.update(Message::DebounceTick);
        assert!(!app.config.show_search_bar);
        assert!(app.search.is_empty());
        assert_eq!(
            app.editor.archives()[0].selected_indices.len(),
            2,
            "hiding the bar must reveal every entry again"
        );

        let _ = app.update(Message::ToggleSearchBar(true));
        assert!(app.config.show_search_bar);
    }

    #[test]
    fn focus_search_shortcut_reveals_a_hidden_search_bar() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::ToggleSearchBar(false));
        assert!(!app.config.show_search_bar);

        let _ = app.handle_shortcut(Shortcut::FocusSearch);
        assert!(app.config.show_search_bar);
        assert!(app.search_focused);
    }

    #[test]
    fn typing_computes_fuzzy_predictions() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::SearchChanged("firs".to_string()));
        let _ = app.update(Message::DebounceTick);

        assert!(app.predictions_open());
        assert_eq!(
            app.search_predictions,
            vec![(0, "first.dff".to_string())]
        );
        assert!(app.did_you_mean.is_none());
    }

    #[test]
    fn no_matches_surface_did_you_mean() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::SearchChanged("fistr".to_string()));
        let _ = app.update(Message::DebounceTick);

        assert!(app.search_predictions.is_empty());
        assert_eq!(
            app.did_you_mean,
            Some((0, "first.dff".to_string()))
        );
        assert!(app.predictions_open());
    }

    #[test]
    fn keyboard_navigates_and_commits_predictions() {
        let mut app = test_app_with_entries();
        app.editor.archives_mut()[0]
            .entries
            .push(EntryInfo::new("firstaid.dff"));

        let _ = app.update(Message::SearchChanged("firs".to_string()));
        let _ = app.update(Message::DebounceTick);
        assert_eq!(app.search_predictions.len(), 2);

        // Down twice: clamps at the last row.
        let _ = app.update(Message::SearchPredictMove(1));
        assert_eq!(app.prediction_index, Some(0));
        let _ = app.update(Message::SearchPredictMove(1));
        assert_eq!(app.prediction_index, Some(1));
        let _ = app.update(Message::SearchPredictMove(1));
        assert_eq!(app.prediction_index, Some(1));

        // Enter commits the highlighted prediction.
        let _ = app.update(Message::SearchPredictCommit);
        assert_eq!(app.search, "firstaid.dff");
        assert_eq!(app.editor.selected_entry(), Some(2));
        assert!(!app.predictions_open(), "dropdown closes after commit");
    }

    #[test]
    fn escape_dismisses_predictions_until_query_changes() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::SearchChanged("firs".to_string()));
        let _ = app.update(Message::DebounceTick);
        assert!(app.predictions_open());

        let _ = app.update(Message::SearchPredictDismiss);
        assert!(!app.predictions_open());

        // Typing again re-opens the dropdown.
        let _ = app.update(Message::SearchChanged("first".to_string()));
        let _ = app.update(Message::DebounceTick);
        assert!(app.predictions_open());
    }

    #[test]
    fn prediction_keys_noop_without_dropdown() {
        let mut app = test_app_with_entries();
        // No query, no focus: all prediction messages must be inert.
        let _ = app.update(Message::SearchPredictMove(1));
        let _ = app.update(Message::SearchPredictCommit);
        let _ = app.update(Message::SearchPickDidYouMean);
        assert_eq!(app.prediction_index, None);
        assert_eq!(app.editor.selected_entry(), None);
    }

    #[test]
    fn shortcuts_are_ignored_while_a_modal_is_open() {
        let mut app = test_app();
        app.show_about = true;

        let _ = app.update(Message::ShortcutPressed(Shortcut::SwitchTab(
            InspectorTab::Model3D,
        )));
        let _ = app.update(Message::SearchFocusChanged(false));
        let follow_up = drain_task(app.update(Message::RenameFocusChanged(false)));
        assert!(follow_up.is_empty(), "modal must swallow the shortcut");
        assert_eq!(app.selected_inspector_tab, InspectorTab::Export);

        app.show_about = false;
        let _ = app.update(Message::ShortcutPressed(Shortcut::SwitchTab(
            InspectorTab::Model3D,
        )));
        let _ = app.update(Message::SearchFocusChanged(false));
        let follow_up = drain_task(app.update(Message::RenameFocusChanged(false)));
        assert_eq!(follow_up.len(), 1);
        assert!(matches!(
            follow_up[0],
            Message::Viewer3dSelectTab(InspectorTab::Model3D)
        ));
    }

    #[test]
    fn middle_click_starts_native_autoscroll_only_over_the_entry_table() {
        let mut app = test_app_with_entries();

        let _ = app.update(Message::AutoScrollStarted);
        assert!(!app.autoscroll, "other panels must not enter table autoscroll");

        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);
        assert!(app.autoscroll);
        assert!(app.autoscroll_notice_shown);
        assert!(app.toast_extended_duration);
    }

    #[test]
    fn entry_table_keeps_its_scrollable_tree_slot_when_autoscroll_starts() {
        let mut app = test_app_with_entries();
        let mut tree = {
            let table = app.build_entry_table();
            iced::advanced::widget::Tree::new(&table)
        };
        let scrollable_tag = tree.children[2].children[0].children[0].tag;

        app.autoscroll = true;
        let table = app.build_entry_table();
        tree.diff(&table);

        assert_eq!(tree.children.len(), 3);
        assert_eq!(tree.children[2].children.len(), 1);
        assert_eq!(tree.children[2].children[0].children.len(), 1);
        assert_eq!(tree.children[2].children[0].children[0].tag, scrollable_tag);
    }

    #[test]
    fn transient_notifications_do_not_recreate_the_workspace_tree() {
        let mut app = test_app_with_entries();
        let mut tree = {
            let view = app.view();
            iced::advanced::widget::Tree::new(&view)
        };
        let root_tag = tree.tag;
        let base_tag = tree.children[0].tag;

        // A toast is rendered as a temporary layer. Its appearance must not
        // replace the base workspace, because that would discard native
        // Scrollable state (including an active middle-click autoscroll).
        app.toast = Some("Saved".to_string());
        app.toast_reveal_text = Some("Saved".to_string());
        {
            let view = app.view();
            tree.diff(&view);
        }

        assert_eq!(tree.tag, root_tag);
        assert_eq!(tree.children[0].tag, base_tag);
        assert!(tree.children.len() > 1);

        // Dismissing the toast must preserve the same root and base slots as
        // well; otherwise the scroll position would jump when the fade ends.
        app.toast = None;
        app.toast_reveal_text = None;
        {
            let view = app.view();
            tree.diff(&view);
        }

        assert_eq!(tree.tag, root_tag);
        assert_eq!(tree.children[0].tag, base_tag);
    }

    #[test]
    fn middle_click_again_stops_native_autoscroll_without_rewinding() {
        let mut app = test_app_with_entries();
        app.scroll_y = 500.0;
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);
        assert!(app.autoscroll);

        let _ = app.update(Message::AutoScrollStarted);
        assert!(!app.autoscroll);
        assert_eq!(app.scroll_y, 500.0);
    }

    #[test]
    fn native_autoscroll_scroll_updates_preserve_the_active_indicator_state() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);

        let _ = app.update(Message::ScrollOffsetChanged {
            y: 200.0,
            max_y: 1_000.0,
        });
        assert!(app.autoscroll, "native scrolling must not hide its indicator");
        assert_eq!(app.scroll_y, 200.0);
    }

    #[test]
    fn autoscroll_momentum_samples_once_instead_of_accumulating_at_speed() {
        let origin = Point::new(50.0, 100.0);
        let mut momentum = AutoScrollMomentum::default();
        momentum.begin(Some(origin));

        // Holding the cursor far away only refreshes the one live sample.
        // It must not compound a multiplier every animation frame.
        for _ in 0..120 {
            assert!(!momentum.update_pointer(Point::new(50.0, 700.0), true));
        }
        assert!(momentum.tail.is_none());
        assert_eq!(
            momentum.last_live_velocity,
            Some(AutoScrollMomentum::MAX_SOURCE_SPEED)
        );

        assert!(momentum.update_pointer(origin, true));
        let tail = momentum.tail.expect("fast source motion arms one tail");
        assert_eq!(tail.velocity.abs(), AutoScrollMomentum::MAX_INITIAL_SPEED);
        assert!(tail.remaining_distance <= AutoScrollMomentum::MAX_TAIL_DISTANCE);
    }

    #[test]
    fn autoscroll_momentum_tail_is_bounded_and_reaches_rest() {
        let mut app = test_app_with_entries();
        let origin = Point::new(50.0, 100.0);
        let start_offset = 400.0;
        let _ = app.update(Message::ScrollOffsetChanged {
            y: start_offset,
            max_y: 10_000.0,
        });
        let _ = app.update(Message::PointerMoved(origin));
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);
        let _ = app.update(Message::PointerMoved(Point::new(50.0, 700.0)));
        let _ = app.update(Message::PointerMoved(origin));
        assert!(app.autoscroll_momentum.is_active());

        let start = std::time::Instant::now();
        let _ = app.update(Message::AnimationTick(start));
        for step in 1..=20 {
            let _ = app.update(Message::AnimationTick(
                start + Duration::from_millis(50 * step),
            ));
        }

        assert!(app.scroll_y > start_offset);
        assert!(
            app.scroll_y <= start_offset + AutoScrollMomentum::MAX_TAIL_DISTANCE + 0.1,
            "tail traveled too far: {}",
            app.scroll_y - start_offset
        );
        assert!(!app.autoscroll_momentum.is_active());
    }

    #[test]
    fn autoscroll_momentum_waits_for_the_native_viewport_range() {
        let mut app = test_app_with_entries();
        let origin = Point::new(50.0, 100.0);
        let _ = app.update(Message::PointerMoved(origin));
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);
        let _ = app.update(Message::PointerMoved(Point::new(50.0, 700.0)));
        let _ = app.update(Message::PointerMoved(origin));
        let initial_velocity = app
            .autoscroll_momentum
            .tail
            .expect("fast source motion arms one tail")
            .velocity;

        let start = std::time::Instant::now();
        let _ = app.update(Message::AnimationTick(start));
        let _ = app.update(Message::AnimationTick(start + Duration::from_millis(50)));
        assert_eq!(app.scroll_y, 0.0);
        assert_eq!(
            app.autoscroll_momentum
                .tail
                .expect("tail waits for native bounds")
                .velocity,
            initial_velocity
        );

        let _ = app.update(Message::ScrollOffsetChanged {
            y: 0.0,
            max_y: 10_000.0,
        });
        let _ = app.update(Message::AnimationTick(start + Duration::from_millis(100)));
        assert!(app.scroll_y > 0.0);
    }

    #[test]
    fn autoscroll_momentum_cancels_immediately_on_fresh_input_or_toggle() {
        let mut app = test_app_with_entries();
        let origin = Point::new(50.0, 100.0);
        let _ = app.update(Message::ScrollOffsetChanged {
            y: 400.0,
            max_y: 10_000.0,
        });
        let _ = app.update(Message::PointerMoved(origin));
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);
        let _ = app.update(Message::PointerMoved(Point::new(50.0, 700.0)));
        let _ = app.update(Message::PointerMoved(origin));
        assert!(app.autoscroll_momentum.is_active());

        // Moving out of the neutral zone hands control back to Iced's native
        // autoscroll and discards the residual tail before it can add speed.
        let _ = app.update(Message::PointerMoved(Point::new(50.0, 700.0)));
        assert!(!app.autoscroll_momentum.is_active());

        let _ = app.update(Message::PointerMoved(origin));
        assert!(app.autoscroll_momentum.is_active());
        let _ = app.update(Message::ToggleAutoscrollMomentum(false));
        assert!(!app.config.autoscroll_momentum_enabled);
        assert!(!app.autoscroll_momentum.is_active());
    }

    #[test]
    fn autoscroll_stop_keeps_the_current_scroll_position() {
        let mut app = test_app_with_entries();
        app.scroll_y = 500.0;
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);

        let _ = app.update(Message::AutoScrollEnded);
        assert!(!app.autoscroll);
        assert_eq!(app.scroll_y, 500.0);
    }

    #[test]
    fn autoscroll_notice_toast_reads_longer_than_regular_toasts() {
        let mut app = test_app_with_entries();
        let _ = app.update(Message::EntryTableHoverChanged(true));
        let _ = app.update(Message::AutoScrollStarted);
        assert!(app.toast_extended_duration);

        let start = std::time::Instant::now();
        let _ = app.update(Message::AnimationTick(start));
        assert_eq!(app.toast_dismiss_after, Duration::from_millis(6500));

        let _ = app.update(Message::AnimationTick(start + Duration::from_millis(3000)));
        assert!(app.toast.is_some(), "notice must outlive 2.5 s");
        let _ = app.update(Message::AnimationTick(start + Duration::from_millis(7000)));
        assert!(app.toast.is_none(), "notice dismisses after 6.5 s");

        app.toast = Some("quick".to_string());
        let _ = app.update(Message::AnimationTick(start + Duration::from_millis(7100)));
        assert_eq!(app.toast_dismiss_after, Duration::from_millis(2500));
    }

    /// Advance the animation clock in 50 ms steps — the real tick handler
    /// caps `dt` at 50 ms, so single large jumps would under-advance.
    fn tick_n(app: &mut App, start: std::time::Instant, steps: u64) -> std::time::Instant {
        let mut now = start;
        for _ in 0..steps {
            now += Duration::from_millis(50);
            let _ = app.update(Message::AnimationTick(now));
        }
        now
    }

    #[test]
    fn toast_overlay_slides_in_and_fades_out() {
        let mut app = test_app();
        assert!(app.toast_overlay().is_none());

        app.toast = Some("Saved.".to_string());
        let start = std::time::Instant::now();
        let _ = app.update(Message::AnimationTick(start));

        // 50 ms in: mid-reveal.
        let mut now = tick_n(&mut app, start, 1);
        let (text, reveal) = app.toast_overlay().expect("toast should be visible");
        assert_eq!(text, "Saved.");
        assert!(reveal > 0.0 && reveal < 1.0, "mid-reveal: {reveal}");

        // 300 ms in: reveal has completed, snackbar fully shown.
        now = tick_n(&mut app, now, 5);
        let (_, reveal) = app.toast_overlay().expect("toast should still be visible");
        assert!((reveal - 1.0).abs() < 0.01);

        // Dismiss: the text survives until the fade-out finishes.
        app.toast = None;
        now = tick_n(&mut app, now, 1); // fade-out starts
        let (text, _) = app.toast_overlay().expect("fade-out keeps the text");
        assert_eq!(text, "Saved.");
        now = tick_n(&mut app, now, 2); // mid-fade
        let (_, reveal) = app.toast_overlay().expect("mid-fade still visible");
        assert!(reveal < 1.0, "mid-fade: {reveal}");
        let _ = tick_n(&mut app, now, 3); // fade completes and clears
        assert!(app.toast_overlay().is_none());
    }

    #[test]
    fn toast_overlay_is_static_when_motion_is_disabled() {
        let mut app = test_app();
        app.config.motion_enabled = false;

        app.toast = Some("Copied".to_string());
        let now = std::time::Instant::now();
        let _ = app.update(Message::AnimationTick(now));

        let (_, reveal) = app.toast_overlay().expect("toast should be visible");
        assert!((reveal - 1.0).abs() < f32::EPSILON);

        app.toast = None;
        let _ = app.update(Message::AnimationTick(now + Duration::from_millis(50)));
        assert!(app.toast_overlay().is_none());
    }

    #[test]
    fn shimmer_phase_advances_only_while_progress_is_active() {
        let mut app = test_app_with_entries();
        let start = std::time::Instant::now();
        let mut now = tick_n(&mut app, start, 2);
        assert_eq!(app.shimmer_phase, 0.0);

        app.editor.archives()[0].progress.start();
        now = tick_n(&mut app, now, 2);
        assert!(app.shimmer_phase > 0.0, "phase: {}", app.shimmer_phase);

        app.editor.archives()[0].progress.finish();
        let held = app.shimmer_phase;
        let _ = tick_n(&mut app, now, 2);
        assert_eq!(app.shimmer_phase, held);
    }

    #[test]
    fn empty_state_breathes_only_with_no_archives_and_motion() {
        let mut app = test_app();
        let start = std::time::Instant::now();
        let now = tick_n(&mut app, start, 2);
        assert!(app.empty_state_phase > 0.0);

        app.editor.new_archive();
        let held = app.empty_state_phase;
        let _ = tick_n(&mut app, now, 2);
        assert_eq!(app.empty_state_phase, held);

        // Motion disabled: no breathing on the empty state either.
        let mut app = test_app();
        app.config.motion_enabled = false;
        let _ = tick_n(&mut app, start, 2);
        assert_eq!(app.empty_state_phase, 0.0);
    }
}
