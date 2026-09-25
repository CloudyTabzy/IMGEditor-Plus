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
