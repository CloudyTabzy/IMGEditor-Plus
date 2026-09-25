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
