### IMG Editor Plus - English (source language).
###
### Every message here becomes a typed function in `i18n::t` at build time
### (`menu-file-new` -> `t::menu_file_new(shortcut)`); see i18n/README.md.
### Brand, format and file names (IMG Editor Plus, TXD, DFF, .img) are not
### translated. Keyboard shortcuts arrive already formatted ("Ctrl + N").

## Menu bar: root menus

menu-file = File
menu-recent = Recent
menu-edit = Edit
menu-selection = Selection
menu-view = View
menu-themes = Themes
menu-help = Help
menu-language = Language

## File menu

menu-file-new = New ({ $shortcut })
menu-file-open = Open… ({ $shortcut })
menu-file-save = Save ({ $shortcut })
menu-file-save-as = Save as… ({ $shortcut })
menu-file-pack = Pack archive
menu-file-set-game-folder = Set game folder…
menu-file-reset-game-folder = Reset game folder
menu-file-close-tab = Close tab ({ $shortcut })
menu-file-sort-by = Sort by…

## Recent menu

menu-recent-empty = No recent files

## Edit menu

menu-edit-import = Import ({ $shortcut })
menu-edit-import-folder = Import folder
menu-edit-export-all = Export all ({ $shortcut })
menu-edit-export-selected = Export selected ({ $shortcut })
menu-edit-export-list = Export as list ({ $shortcut })
menu-edit-compare-list = Compare with list ({ $shortcut })
menu-edit-load-agr = Load .agr animation file…

## Selection menu

menu-selection-all = Select all ({ $shortcut })
menu-selection-invert = Invert selection ({ $shortcut })
menu-selection-clear = Clear selection ({ $shortcut })
menu-selection-delete = Delete selected ({ $shortcut })

## View menu (toggles; the on/off marker is added by the code)

menu-view-navigation-gizmo = Navigation gizmo
menu-view-search-bar = Search bar
menu-view-search-selection-context = Search selection context
menu-view-literal-file-types = Literal file types
menu-view-highlight-validator-rows = Highlight validator rows
menu-view-context-accumulates = Right-click adds to selection
menu-view-autoscroll-momentum = Autoscroll momentum
menu-view-motion-effects = Motion effects
menu-view-selection-pulse = Selection pulse
menu-view-click-ripples = Click ripples
menu-view-icon-micro-motion = Icon micro-motion
menu-view-animation-demo = Animation demo (synthetic)
menu-view-explorer-association = Open .img/.dir from Explorer

## Themes menu (named themes such as Catppuccin Mocha keep their names)

theme-dark = Dark
theme-light = Light

## Help menu

menu-help-check-updates = Check for updates ({ $shortcut })
menu-help-repository = Visit repository
menu-help-about = About

## Language menu (each language is listed under its own name)

# $language is the detected system language's own name, e.g. "Español".
menu-language-system = System language ({ $language })
menu-language-pseudo = Pseudo-locale (layout test)

## Entry context menu (right-click on an archive entry)

# Shown under the entry name when the right-click extends a selection.
context-more-selected =
    { $count ->
        [one] +{ $count } more selected
       *[other] +{ $count } more selected
    }
context-play-animation = Play animation
context-open-3d = Open in 3D viewer
context-open-external = Open in external viewer
context-view-textures = View textures
context-export-companion-textures = Export companion NFT textures
context-export-embedded-textures = Export embedded textures
context-export = Export
context-rename = Rename
context-copy-name = Copy name
context-delete = Delete

## Shared dialog buttons and labels

button-close = Close
button-cancel = Cancel
button-save = Save
button-discard = Discard
button-apply = Apply
button-reset = Reset
button-convert = Convert
button-planning = Planning…
field-format = Format:
field-name = Name:
# Stands in for a game name in sentences like "checked against { $target }".
target-unset = the target
target-unset-selected = the selected target

## About dialog

dialog-about-title = About
about-body =
    IMG Editor Plus v{ $version }

    A pure Rust desktop editor for GTA IMG archives.

    Made by CloudyTabzy & Agents
    Based on the original IMG Editor by Grinch_
    (https://github.com/user-grinch/IMGEditor)

    Supported formats:
    - GTA III
    - GTA Vice City
    - GTA San Andreas
    - Bully Scholarship Edition
about-visit-repository = Visit repository

## Welcome dialog

dialog-welcome-title = Welcome
welcome-heading = Welcome to { $app } v{ $version }
welcome-tagline = A GTA archive editor for III, VC, San Andreas, Bully SE.
welcome-dont-show = Don't show this message again
welcome-disable-updates = Disable update checking
welcome-get-started = Get started

## Unsupported format dialog

dialog-unsupported-title = Unsupported format
unsupported-body = IMG format not supported.
unsupported-path = Path: { $path }
unsupported-supported = Supported formats: GTA III, Vice City, San Andreas, Bully SE.

## Texture converter dialogs (replace texture, image as TXD, bulk convert)

format-option-native = { $format } (native)
format-option-opt-in = { $format } (opt-in)
dxt-high-quality = High-quality DXT (cluster fit, slower)
preview-full-quality = View at full quality
preview-full-quality-title = { $label } — full quality
preview-navigation-hint = Scroll to zoom · drag to pan · Esc to close
preview-current = Current
preview-after = After (encoded)
dialog-target = Target: { $target }
dialog-target-archive = Target: { $target } · { $archive }
dialog-replace-title = Replace texture
replace-summary = Replacing '{ $texture }' - source: { $source } ({ $width }x{ $height })
replace-override-note = Stored in memory as an override; the archive file changes when you save.
replace-confirm = Replace texture
dialog-new-txd-title = Import image as TXD
new-txd-summary = New TXD from { $source } ({ $width }x{ $height })
new-txd-name-placeholder = texture name
new-txd-confirm = Add to archive
dialog-bulk-title = Convert to target dialect
bulk-entry =
    { $entry } - { $count ->
        [one] { $count } texture
       *[other] { $count } textures
    } -> { $formats }
bulk-more-entries =
    { $count ->
        [one] … and { $count } more entry
       *[other] … and { $count } more entries
    }
bulk-summary =
    { $textures ->
        [one] { $textures } texture
       *[other] { $textures } textures
    } across { $entries ->
        [one] { $entries } entry
       *[other] { $entries } entries
    } of { $source } will be re-encoded for the target.
bulk-skipped = { $skipped } already native (skipped), { $failed } unreadable (skipped).
bulk-ignored =
    { $count ->
        [one] { $count } of the selected entries is not a TXD container and stays untouched.
       *[other] { $count } of the selected entries are not TXD containers and stay untouched.
    }
bulk-verbatim-note = Untouched textures and names stay verbatim; the archive changes when you save.

## Compare with list dialog

dialog-compare-title = Compare with list
compare-manifest = Manifest: { $path }
compare-archive =
    Archive: { $archive } · { $count ->
        [one] { $count } archive entry
       *[other] { $count } archive entries
    }
compare-stats =
    { $names ->
        [one] { $names } manifest name
       *[other] { $names } manifest names
    } ({ $unique } unique) · { $matched } matched · { $missing } missing ({ $missing-unique } unique)
compare-case-sensitive = Case-sensitive matching
compare-show-archive-only = Show archive-only entries
compare-duplicate-lines =
    { $count ->
        [one] { $count } duplicate manifest line
       *[other] { $count } duplicate manifest lines
    }
compare-blank-lines =
    { $count ->
        [one] { $count } blank line ignored
       *[other] { $count } blank lines ignored
    }
compare-missing-heading = Missing from archive ({ $count })
compare-no-missing = No missing entries found.
compare-more-missing =
    { $count ->
        [one] … and { $count } more missing name
       *[other] … and { $count } more missing names
    }
compare-archive-only-heading = Archive-only entries ({ $count })
compare-no-archive-only = No archive-only entries found.
compare-more-archive-only =
    { $count ->
        [one] … and { $count } more archive-only name
       *[other] … and { $count } more archive-only names
    }
compare-copy-missing = Copy missing names
compare-running-heading = Comparing entry list
compare-running-body = Reading { $manifest } and checking it against { $archive }…

## Save check dialog

dialog-save-check-title = Save check
save-check-summary =
    { $textures ->
        [one] { $textures } texture
       *[other] { $textures } textures
    }, { $entries ->
        [one] { $entries } entry
       *[other] { $entries } entries
    } - checked against { $target }.
save-check-counts = { $fine } native/supported · { $convertible } convertible (lossless) · { $incompatible } incompatible · { $unknown } unknown
# $note is a technical detail and stays in English.
save-check-container = Container: { $note }
save-check-anomaly = { $code }: { $count } (e.g. { $example })
save-check-broken-headers = Broken headers (fixable without re-encoding):
save-check-warnings =
    { $count ->
        [one] { $count } warning-level anomaly (reported, not blocking).
       *[other] { $count } warning-level anomalies (reported, not blocking).
    }
save-check-repair =
    Repair inconsistent DXT headers before saving ({ $count ->
        [one] { $count } report
       *[other] { $count } reports
    }, lossless)
save-check-repair-note = Repair patches DXT header fields in place; no pixel is re-encoded.
save-check-verbatim-note = Saving writes every entry verbatim; no texture is re-encoded or converted.
save-check-fix-and-save = Fix & Save
save-check-save-anyway = Save anyway

## Unsaved changes dialog

dialog-unsaved-title = Unsaved changes
unsaved-archive = '{ $archive }' has unsaved changes.
unsaved-archive-note = Closing without saving discards them; the archive file on disk is untouched.
unsaved-window =
    { $count ->
        [one] { $count } archive has unsaved changes: { $archives }
       *[other] { $count } archives have unsaved changes: { $archives }
    }
unsaved-window-note = Quitting now discards them; the files on disk are untouched.
unsaved-discard-and-quit = Discard changes and quit

## Import check dialog

dialog-import-check-title = Import check
import-check-offender = { $name }: { $verdict }
import-check-offender-note = { $name }: { $verdict } - { $note }
import-check-file =
    { $count ->
        [one] { $count } texture
       *[other] { $count } textures
    }: { $detail }
import-check-more =
    { $count ->
        [one] …and { $count } more flagged file.
       *[other] …and { $count } more flagged files.
    }
import-check-summary =
    { $flagged } of { $total ->
        [one] { $total } file
       *[other] { $total } files
    } need a decision for { $target }: { $incompatible ->
        [one] { $incompatible } incompatible texture
       *[other] { $incompatible } incompatible textures
    }, { $unknown } unknown.
import-check-note = Imports are verbatim either way - the format only matters if the game must load these textures.
import-check-import-anyway = Import anyway
import-check-cancel = Cancel import

## Import folder dialog

dialog-folder-import-title = Import folder
folder-import-path = Folder: { $path }
folder-import-summary =
    { $count ->
        [one] { $count } regular file
       *[other] { $count } regular files
    } • { $size }
folder-import-top-level = Only files directly inside this folder are included; subfolders are not scanned.
folder-import-no-duplicates = No duplicate names detected.
folder-import-duplicates =
    { $count ->
        [one] { $count } duplicate name detected. Choose how to handle it.
       *[other] { $count } duplicate names detected. Choose how to handle them.
    }
folder-import-skipped =
    { $count ->
        [one] { $count } item could not be inspected and will be skipped.
       *[other] { $count } items could not be inspected and will be skipped.
    }
folder-import-files = Import files
folder-import-skip-duplicates = Import (skip duplicates)
folder-import-replace-duplicates = Replace duplicates

## Update check dialog

dialog-update-title = Update check
update-available = Update available: { $version }
update-latest = You are using the latest version.
# $error is a technical detail and stays in English.
update-failed = Update check failed: { $error }
update-dont-show = Do not show this message again
update-open-releases = Open releases

## Validate textures dialog

dialog-validator-title = Validate textures
validator-intro = Pick the game this archive targets. The validator flags every texture outside that engine's retail-accepted formats, and lists the unknowns that could plausibly load but are not game-native.
validator-last-run = Last run: { $counts }
validator-no-textures = no textures
validator-current-target = current target
validator-validate-for = Validate for { $game }
validator-native-heading = Native (retail-verified):
validator-unknown-heading = Unknown / not game-native:
validator-hint-likely = Content looks like { $game }
validator-hint-possible = Content possibly { $game }
validator-hint-current = current target: { $game }
validator-use-as-target = Use { $game } as target
validator-already-target = (already the target)
validator-highlight-rows = Highlight rows
legend-native = native
legend-native-description = Authored by this engine - no action needed.
legend-supported = supported / convertible
legend-supported-description = Loads, but is not the game's data dialect; a lossless rewrite may be offered.
legend-lossy = lossy convert
legend-lossy-description = Can be used only after a pixel-changing conversion (compression or quantization).
legend-unknown = unknown
legend-unknown-description = No evidence either way - not known to be incompatible. Treat with care.
legend-incompatible = incompatible
legend-incompatible-description = The selected engine cannot consume this format.

## Sort by dialog

dialog-sort-title = Sort by — { $archive }
dialog-sort-title-no-archive = Sort by — (no archive open)
sort-empty = No keys yet. Add a key below to start sorting.
sort-intro = Set priority rules, then apply them to the current archive.
sort-select-key = Select key…
sort-add-key = + Add key
sort-add-key-max = + Add key (max reached)
sort-keys-active =
    { $active } of { $total ->
        [one] { $total } key active
       *[other] { $total } keys active
    }
sort-preview-heading = Live preview (first 10 entries)
sort-preview-empty = (no entries in the current archive)
sort-apply-preset = Apply preset…
sort-ascending-short = Asc ▲
sort-descending-short = Desc ▼
sort-key-name = Name
sort-key-extension = Extension
sort-key-type = Type
sort-key-size = Size
sort-key-offset = Offset
sort-key-ide-file = IDE file
sort-key-col-file = COL file
sort-preset-name-az = Name (A→Z)
sort-preset-name-za = Name (Z→A)
sort-preset-type-then-name = Type, then name
sort-preset-size-desc = Size (big → small)
sort-preset-offset-asc = Offset (low → high)

## Windows file dialogs (titles and file-type filters)

file-dialog-open-archive = Open IMG archive
file-dialog-filter-img = IMG archive
file-dialog-compare = Compare with entry list
file-dialog-filter-entry-list = IMG entry list
file-dialog-open-agr = Open Bully animation group
file-dialog-filter-agr = Bully animation group
file-dialog-import-files = Import files
file-dialog-filter-importable = Importable files
file-dialog-import-folder = Select folder to import
file-dialog-game-folder = Select the game folder
file-dialog-choose-image = Choose an image
file-dialog-filter-images = Images
file-dialog-save-archive = Save IMG archive
file-dialog-export-list = Export entry list
file-dialog-export-folder = Select export folder

## Toasts: archives and selection

toast-no-archive-selected = No archive selected.
toast-archive-closed = The archive is no longer open.
toast-target-archive-closed = The target archive is no longer open.
toast-target-archive-deselected = The target archive is no longer selected.
toast-validated-archive-closed = The validated archive was closed.
toast-entry-unavailable = The selected entry is no longer available.
toast-task-running = Another task is still running.
toast-operation-running = An archive operation is already running.
toast-open-archive-to-validate = Open an archive first to validate it.
toast-open-archive-to-import = Open an archive first to import into it.
toast-open-archive-to-import-folder = Open an archive first to import a folder.
toast-open-img-for-model = Open an IMG archive first; the model comes from it.
toast-already-open = Already open: { $path }
# $error is a technical detail and stays in English.
toast-open-failed = Failed to open archive: { $error }
toast-file-gone = File no longer exists: { $path }
toast-file-not-found = File not found: { $path }
toast-compare-empty-archive = The selected archive has no entries to compare.

## Toasts: saving and packing

toast-archive-saved = Archive saved.
toast-save-cancelled = Save cancelled.
toast-save-failed = Save failed: { $error }
toast-save-before-pack = Save the archive before packing it.
toast-packed = Archive packed — reclaimed { $reclaimed } ({ $size } on disk).
toast-packed-nothing = Archive packed — no space reclaimed ({ $size } on disk).
toast-pack-failed = Pack failed: { $error }
toast-headers-repaired =
    { $count ->
        [one] Repaired { $count } texture header; saving.
       *[other] Repaired { $count } texture headers; saving.
    }

## Toasts: game folder

toast-game-folder-set = Game folder for { $archive }: { $path }
toast-game-folder-automatic = Game folder for { $archive }: { $path } (automatic)
toast-game-folder-none = { $archive } has no game folder.
toast-game-folder-needs-save = Save the archive first; the game folder is stored per archive file.

## Toasts: 3D viewer and animation

toast-select-model = Select a NIF, DFF, or COL entry first.
toast-viewer-unsupported = The in-app 3D viewer supports .nif, .dff, and .col ({ $entry }).
toast-select-ifp = Select an .ifp entry first.
toast-no-model-for-agr = No matching .nif model found for { $animation } in this archive.
toast-no-model-for-agr-open = No matching .nif model found for { $animation } in the open archive.
toast-loading-animation = Loading { $animation } on { $model }…
toast-loading-animation-hxd = Loading { $animation } on { $model }… (HXD catalog found)
toast-playback-stalled = Playback paused after a long stall.
toast-demo-loaded = Synthetic animation demo loaded (no game data).
toast-demo-closed = Animation demo closed.
toast-animation-ready = Animation ready: { $summary }
toast-animation-failed = Animation load failed: { $error }
toast-3d-load-failed = 3D load failed: { $error }
anim-summary-ifp =
    { $animation } on { $model } ({ $clips ->
        [one] { $clips } clip
       *[other] { $clips } clips
    }, GTA IFP)
anim-summary-agr =
    { $animation } on { $model } ({ $clips ->
        [one] { $clips } clip
       *[other] { $clips } clips
    })
anim-summary-agr-named =
    { $animation } on { $model } ({ $clips ->
        [one] { $clips } clip
       *[other] { $clips } clips
    }, HXD-named)

## Toasts: textures and conversion

toast-select-texture = Select a texture entry first.
toast-no-target = No target set for this archive. Pick the game it is for in Validate textures.
toast-target-error = { $error } Pick the game this archive is for in Validate textures.
toast-replace-busy = A replacement is already being prepared.
toast-preparing-replacement = Preparing replacement…
toast-replace-failed = Replace failed: { $error }
toast-import-busy = An import is already being prepared.
toast-preparing-import = Preparing import…
toast-txd-name-required = Give the new TXD a name.
toast-entry-exists = An entry named '{ $name }' already exists.
toast-entry-added = Added '{ $name }' - save the archive to write it.
toast-set-target-first = Set a game target first (Validate textures).
toast-select-entries-to-convert = Select the entries to convert first.
toast-archive-changed-planning = The archive changed while the conversion was being planned; please retry.
toast-convert-only-txd = Only TXD entries can be converted - none of the selected entries are TXD texture containers.
toast-convert-all-native = Every selected texture is already native for the target.
toast-archive-changed-after-plan = The archive changed after this conversion was planned; please retry.
toast-archive-changed-during = The archive changed during conversion; stale results were discarded.
toast-converted =
    { $count ->
        [one] Converted { $count } entry - save the archive to write it.
       *[other] Converted { $count } entries - save the archive to write them.
    }
toast-conversion-failed = Conversion failed: { $error }
toast-no-decoded-textures = No decoded textures to export.
toast-decoded =
    { $count ->
        [one] Decoded { $count } texture
       *[other] Decoded { $count } textures
    }
toast-decoded-not-retained =
    { $count ->
        [one] Decoded { $count } texture, but the preview could not be kept
       *[other] Decoded { $count } textures, but the preview could not be kept
    }
toast-pick-embedded-folder = Pick a folder to export embedded textures from { $model }
toast-basename-unknown = Cannot determine the base name of { $entry }
toast-read-failed = Failed to read { $name }: { $error }

## Toasts: import and export

toast-import-cancelled = Import cancelled.
toast-imported =
    { $count ->
        [one] Imported { $count } file.
       *[other] Imported { $count } files.
    }
toast-imported-unchecked =
    { $count ->
        [one] Imported { $count } file - no validator target set, formats were not checked.
       *[other] Imported { $count } files - no validator target set, formats were not checked.
    }
toast-import-failed = Import failed: { $error }
toast-no-files-in-folder = No regular files found in { $folder }.
toast-folder-scan-failed = Folder scan failed: { $error }
toast-folder-import-failed = Folder import failed: { $error }
toast-folder-import-done = Folder import complete: { $imported } imported, { $skipped } skipped, { $failed } failed.
toast-folder-import-cancelled = Folder import cancelled: { $imported } imported, { $skipped } skipped, { $failed } failed.
toast-see-log = See the archive log for details.
toast-exported =
    { $count ->
        [one] Exported { $count } entry.
       *[other] Exported { $count } entries.
    }
toast-export-failed = Export failed: { $error }
toast-entry-list-exported =
    { $count ->
        [one] Exported { $count } entry name to { $path }.
       *[other] Exported { $count } entry names to { $path }.
    }
toast-entry-list-export-failed = Entry-list export failed: { $error }
toast-compare-failed = Entry-list comparison failed: { $error }
toast-no-missing-to-copy = There are no missing entries to copy.
toast-copied-missing =
    { $count ->
        [one] Copied { $count } missing entry name.
       *[other] Copied { $count } missing entry names.
    }

## Toasts: clipboard, dragging and other actions

toast-copied-entry-details = Copied the selected entry's details.
toast-copied-logs = Copied the log.
toast-copied-name = Copied name: { $name }
toast-drag-cancelled = Drag cancelled.
toast-moved-entries =
    { $count ->
        [one] Moved { $count } entry to archive #{ $archive }.
       *[other] Moved { $count } entries to archive #{ $archive }.
    }
toast-autoscroll = Autoscroll active: move the pointer to scroll. Click, middle-click, right-click, use the wheel, or press a key to stop.
toast-association-added = IMG Editor Plus is now in Explorer's "Open with" for .img/.dir. To make it the default, pick it in the Settings page that opened.
toast-association-removed = Removed the .img/.dir association.
toast-association-failed = File association failed: { $error }
toast-validation-cancelled = Validation cancelled.
toast-validation-failed = Validation failed: { $error }
toast-no-compat-issues = No compatibility issues found - { $summary }
# $verdicts is the per-verdict count list, e.g. "native 12, unknown 1".
validation-summary =
    Validated { $txds ->
        [one] { $txds } TXD
       *[other] { $txds } TXDs
    } ({ $textures ->
        [one] { $textures } texture
       *[other] { $textures } textures
    }) for { $game }: { $verdicts }; { $errors ->
        [one] { $errors } error
       *[other] { $errors } errors
    }, { $warnings ->
        [one] { $warnings } warning
       *[other] { $warnings } warnings
    }

## Archive log (the Logs box in the Export tab)

log-viewer-ready = In-app 3D viewer ready
log-viewer-ready-cached = In-app 3D viewer ready (cached)
log-viewer-opened = 3D viewer opened: { $name }
log-viewer-failed = 3D viewer failed: { $reason }
log-viewer-closed = 3D viewer closed
log-external-viewer = Opening external 3D viewer for { $name }
log-exported =
    { $count ->
        [one] Exported { $count } entry
       *[other] Exported { $count } entries
    }
log-export-failed = Export failed: { $error }
log-entry-list-exported =
    { $count ->
        [one] Exported entry list ({ $count } name) to { $path }
       *[other] Exported entry list ({ $count } names) to { $path }
    }
log-compat-check = Compatibility check: { $summary }
log-decoded =
    { $count ->
        [one] Decoded { $count } texture preview
       *[other] Decoded { $count } texture previews
    }
log-texture-export-failed = { $model } export failed: { $error }
# "Exported <what>" in the Export tab's recent list.
recent-exported = Exported { $what }
recent-exported-files =
    { $count ->
        [one] { $count } file
       *[other] { $count } files
    }

## Empty workspace pro tips (they name menus and keys; keep those in step
## with the translated menu labels)

pro-tip-label = Pro tip:
pro-tip-search = Press Ctrl+F to focus Search, then use Up/Down and Enter to jump to a match.
pro-tip-search-context = Search predictions reveal a match in its archive context; View → Search selection context enables isolated results.
pro-tip-context-menu = Right-click an entry for 3D view, textures, export, rename, and other actions.
pro-tip-autoscroll = Middle-click the entry list for browser-style autoscroll; optional momentum is under View.
pro-tip-tab-keys = Press 1, 2, or 3 to switch to Export, 3D view, or Texture.
pro-tip-close-tab = Middle-click an archive tab to close it quickly.
pro-tip-uv-overlay = Texture UV overlays are available when the selected model supplies matching geometry.
pro-tip-wire-grid = In 3D view, Wire overlay exposes triangle edges and Grid floor helps judge scale.
pro-tip-save-keys = Use Ctrl+S for a quick save and Ctrl+Shift+S to save an archive under a new name.
pro-tip-unique-exports = Exported textures use unique filenames automatically, so batch exports never overwrite one another.
pro-tip-agr = Edit → Load .agr animation file plays an animation in the 3D view; Space toggles playback and ←/→ step frames.
pro-tip-model-picker = While an animation plays, the dock's Model picker re-plays it on any compatible model in the archive.
pro-tip-fullscreen-preview = The expand icon on an import or replace preview opens it fullscreen: scroll to zoom, drag to pan, Esc to close.
pro-tip-bulk-convert = Convert selection to target dialect bulk re-encodes the selected TXDs — pick the game in Validate textures first.
pro-tip-entry-lists = Ctrl+L exports an entry list and Ctrl+P compares one against the archive to spot missing names.
toast-no-dff-to-animate = No DFF model found in this archive to animate.
toast-replace-needs-txd = Replacement works on TXD entries; that entry is not one.
toast-texture-replaced = Texture replaced - save the archive to write it.
toast-bully-texture-writing = Bully (Gamebryo) texture writing is not supported yet.
toast-archive-file-missing = The archive file no longer exists. Use Save as… first.
toast-folder-scan-target-changed = The target archive changed while the folder was being scanned.
toast-folder-import-discarded = Folder import discarded because the target archive changed.
toast-compare-discarded = Comparison discarded because the archive changed; please retry.
toast-drop-needs-archive = Open an archive first to drop non-IMG files into it.
toast-no-game-root = Could not determine the game folder from the archive path.
toast-textures-exported =
    { $count ->
        [one] Exported { $count } texture.
       *[other] Exported { $count } textures.
    }
toast-textures-export-failed = Texture export failed: { $error }
error-write-file = Failed to write { $path }: { $error }
error-read-entry = Failed to read entry: { $error }
error-texture-decode = Texture decode failed: { $error }
error-texture-preview-unsupported = Texture preview supports TXD and NFT entries; '{ $entry }' is not a supported texture container.
