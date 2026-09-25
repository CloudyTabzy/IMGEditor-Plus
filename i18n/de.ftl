### IMG Editor Plus - Deutsch.
###
### Draft translation; pending review by a native speaker.
### Keep message ids and { $variables } exactly as in en.ftl.

## Menu bar: root menus

menu-file = Datei
menu-recent = Zuletzt verwendet
menu-edit = Bearbeiten
menu-selection = Auswahl
menu-view = Ansicht
menu-themes = Designs
menu-help = Hilfe
menu-language = Sprache

## File menu

menu-file-new = Neu ({ $shortcut })
menu-file-open = Öffnen… ({ $shortcut })
menu-file-save = Speichern ({ $shortcut })
menu-file-save-as = Speichern unter… ({ $shortcut })
menu-file-pack = Archiv packen
menu-file-set-game-folder = Spielordner festlegen…
menu-file-reset-game-folder = Spielordner zurücksetzen
menu-file-close-tab = Tab schließen ({ $shortcut })
menu-file-sort-by = Sortieren nach…

## Recent menu

menu-recent-empty = Keine zuletzt verwendeten Dateien

## Edit menu

menu-edit-import = Importieren ({ $shortcut })
menu-edit-import-folder = Ordner importieren
menu-edit-export-all = Alle exportieren ({ $shortcut })
menu-edit-export-selected = Auswahl exportieren ({ $shortcut })
menu-edit-export-list = Als Liste exportieren ({ $shortcut })
menu-edit-compare-list = Mit Liste vergleichen ({ $shortcut })
menu-edit-load-agr = .agr-Animationsdatei laden…

## Selection menu

menu-selection-all = Alles auswählen ({ $shortcut })
menu-selection-invert = Auswahl umkehren ({ $shortcut })
menu-selection-clear = Auswahl aufheben ({ $shortcut })
menu-selection-delete = Auswahl löschen ({ $shortcut })

## View menu (toggles; the on/off marker is added by the code)

menu-view-navigation-gizmo = Navigations-Gizmo
menu-view-search-bar = Suchleiste
menu-view-search-selection-context = Auswahlkontext in der Suche
menu-view-literal-file-types = Literale Dateitypen
menu-view-highlight-validator-rows = Validator-Zeilen hervorheben
menu-view-context-accumulates = Rechtsklick erweitert die Auswahl
menu-view-autoscroll-momentum = Autoscroll-Trägheit
menu-view-motion-effects = Bewegungseffekte
menu-view-selection-pulse = Auswahl-Pulsieren
menu-view-click-ripples = Klick-Wellen
menu-view-icon-micro-motion = Icon-Mikrobewegung
menu-view-animation-demo = Animationsdemo (synthetisch)
menu-view-explorer-association = .img/.dir aus dem Datei-Explorer öffnen

## Themes menu (named themes such as Catppuccin Mocha keep their names)

theme-dark = Dunkel
theme-light = Hell

## Help menu

menu-help-check-updates = Nach Updates suchen ({ $shortcut })
menu-help-repository = Repository besuchen
menu-help-about = Über

## Language menu (each language is listed under its own name)

# $language is the detected system language's own name, e.g. "Español".
menu-language-system = Systemsprache ({ $language })
menu-language-pseudo = Pseudolokalisierung (Layout-Test)

## Entry context menu (right-click on an archive entry)

# Shown under the entry name when the right-click extends a selection.
context-more-selected =
    { $count ->
        [one] +{ $count } weiterer ausgewählt
       *[other] +{ $count } weitere ausgewählt
    }
context-play-animation = Animation abspielen
context-open-3d = Im 3D-Viewer öffnen
context-open-external = In externem Viewer öffnen
context-view-textures = Texturen anzeigen
context-export-companion-textures = Zugehörige NFT-Texturen exportieren
context-export-embedded-textures = Eingebettete Texturen exportieren
context-export = Exportieren
context-rename = Umbenennen
context-copy-name = Namen kopieren
context-delete = Löschen

## Shared dialog buttons and labels

button-close = Schließen
button-cancel = Abbrechen
button-save = Speichern
button-discard = Verwerfen
button-apply = Anwenden
button-reset = Zurücksetzen
button-convert = Konvertieren
button-planning = Wird geplant…
field-format = Format:
field-name = Name:
# Stands in for a game name in sentences like "checked against { $target }".
target-unset = das Ziel
target-unset-selected = das ausgewählte Ziel

## About dialog

dialog-about-title = Über
about-body =
    IMG Editor Plus v{ $version }

    Ein Desktop-Editor in reinem Rust für GTA-IMG-Archive.

    Erstellt von CloudyTabzy & Agenten
    Basierend auf dem ursprünglichen IMG Editor von Grinch_
    (https://github.com/user-grinch/IMGEditor)

    Unterstützte Formate:
    - GTA III
    - GTA Vice City
    - GTA San Andreas
    - Bully Scholarship Edition
about-visit-repository = Repository besuchen

## Welcome dialog

dialog-welcome-title = Willkommen
welcome-heading = Willkommen bei { $app } v{ $version }
welcome-tagline = Ein GTA-Archiveditor für III, VC, San Andreas, Bully SE.
welcome-dont-show = Diese Nachricht nicht mehr anzeigen
welcome-disable-updates = Updateprüfung deaktivieren
welcome-get-started = Loslegen

## Unsupported format dialog

dialog-unsupported-title = Nicht unterstütztes Format
unsupported-body = IMG-Format nicht unterstützt.
unsupported-path = Pfad: { $path }
unsupported-supported = Unterstützte Formate: GTA III, Vice City, San Andreas, Bully SE.

## Texture converter dialogs (replace texture, image as TXD, bulk convert)

format-option-native = { $format } (nativ)
format-option-opt-in = { $format } (optional)
dxt-high-quality = Hochwertiges DXT (Cluster-Fit, langsamer)
preview-full-quality = In voller Qualität anzeigen
preview-full-quality-title = { $label } — volle Qualität
preview-navigation-hint = Scrollen zum Zoomen · Ziehen zum Verschieben · Esc zum Schließen
preview-current = Aktuell
preview-after = Danach (kodiert)
dialog-target = Ziel: { $target }
dialog-target-archive = Ziel: { $target } · { $archive }
dialog-replace-title = Textur ersetzen
replace-summary = „{ $texture }“ wird ersetzt – Quelle: { $source } ({ $width }x{ $height })
replace-override-note = Wird im Speicher als Überschreibung gehalten; die Archivdatei ändert sich beim Speichern.
replace-confirm = Textur ersetzen
dialog-new-txd-title = Bild als TXD importieren
new-txd-summary = Neues TXD aus { $source } ({ $width }x{ $height })
new-txd-name-placeholder = Texturname
new-txd-confirm = Zum Archiv hinzufügen
dialog-bulk-title = In den Zieldialekt konvertieren
bulk-entry =
    { $entry } - { $count ->
        [one] { $count } Textur
       *[other] { $count } Texturen
    } -> { $formats }
bulk-more-entries =
    { $count ->
        [one] … und { $count } weiterer Eintrag
       *[other] … und { $count } weitere Einträge
    }
bulk-summary =
    { $textures ->
        [one] { $textures } Textur
       *[other] { $textures } Texturen
    } in { $entries ->
        [one] { $entries } Eintrag
       *[other] { $entries } Einträge
    } aus { $source } werden für das Ziel neu kodiert.
bulk-skipped = { $skipped } bereits nativ (übersprungen), { $failed } nicht lesbar (übersprungen).
bulk-ignored =
    { $count ->
        [one] { $count } der ausgewählten Einträge ist kein TXD-Container und bleibt unverändert.
       *[other] { $count } der ausgewählten Einträge sind keine TXD-Container und bleiben unverändert.
    }
bulk-verbatim-note = Unveränderte Texturen und Namen bleiben unverändert; das Archiv ändert sich beim Speichern.

## Compare with list dialog

dialog-compare-title = Mit Liste vergleichen
compare-manifest = Manifest: { $path }
compare-archive =
    Archiv: { $archive } · { $count ->
        [one] { $count } Archiveintrag
       *[other] { $count } Archiveinträge
    }
compare-stats =
    { $names ->
        [one] { $names } Manifestname
       *[other] { $names } Manifestnamen
    } ({ $unique } eindeutig) · { $matched } zugeordnet · { $missing } fehlend ({ $missing-unique } eindeutig)
compare-case-sensitive = Groß-/Kleinschreibung beachten
compare-show-archive-only = Nur im Archiv vorhandene Einträge anzeigen
compare-duplicate-lines =
    { $count ->
        [one] { $count } doppelte Manifestzeile
       *[other] { $count } doppelte Manifestzeilen
    }
compare-blank-lines =
    { $count ->
        [one] { $count } Leerzeile ignoriert
       *[other] { $count } Leerzeilen ignoriert
    }
compare-missing-heading = Im Archiv fehlend ({ $count })
compare-no-missing = Keine fehlenden Einträge gefunden.
compare-more-missing =
    { $count ->
        [one] … und { $count } weiterer fehlender Name
       *[other] … und { $count } weitere fehlende Namen
    }
compare-archive-only-heading = Nur im Archiv vorhanden ({ $count })
compare-no-archive-only = Keine nur im Archiv vorhandenen Einträge gefunden.
compare-more-archive-only =
    { $count ->
        [one] … und { $count } weiterer nur im Archiv vorhandener Name
       *[other] … und { $count } weitere nur im Archiv vorhandene Namen
    }
compare-copy-missing = Fehlende Namen kopieren
compare-running-heading = Eintragsliste wird verglichen
compare-running-body = { $manifest } wird gelesen und mit { $archive } abgeglichen…

## Save check dialog

dialog-save-check-title = Prüfung vor dem Speichern
save-check-summary =
    { $textures ->
        [one] { $textures } Textur
       *[other] { $textures } Texturen
    }, { $entries ->
        [one] { $entries } Eintrag
       *[other] { $entries } Einträge
    } – geprüft gegen { $target }.
save-check-counts = { $fine } nativ/unterstützt · { $convertible } konvertierbar (verlustfrei) · { $incompatible } inkompatibel · { $unknown } unbekannt
# $note is a technical detail and stays in English.
save-check-container = Container: { $note }
save-check-anomaly = { $code }: { $count } (z. B. { $example })
save-check-broken-headers = Beschädigte Header (ohne Neukodierung reparierbar):
save-check-warnings =
    { $count ->
        [one] { $count } Anomalie auf Warnstufe (gemeldet, nicht blockierend).
       *[other] { $count } Anomalien auf Warnstufe (gemeldet, nicht blockierend).
    }
save-check-repair =
    Inkonsistente DXT-Header vor dem Speichern reparieren ({ $count ->
        [one] { $count } Bericht
       *[other] { $count } Berichte
    }, verlustfrei)
save-check-repair-note = Die Reparatur korrigiert DXT-Headerfelder direkt; kein Pixel wird neu kodiert.
save-check-verbatim-note = Beim Speichern wird jeder Eintrag unverändert geschrieben; keine Textur wird neu kodiert oder konvertiert.
save-check-fix-and-save = Reparieren und speichern
save-check-save-anyway = Trotzdem speichern

## Unsaved changes dialog

dialog-unsaved-title = Nicht gespeicherte Änderungen
unsaved-archive = „{ $archive }“ hat nicht gespeicherte Änderungen.
unsaved-archive-note = Schließen ohne Speichern verwirft sie; die Archivdatei auf der Festplatte bleibt unverändert.
unsaved-window =
    { $count ->
        [one] { $count } Archiv hat nicht gespeicherte Änderungen: { $archives }
       *[other] { $count } Archive haben nicht gespeicherte Änderungen: { $archives }
    }
unsaved-window-note = Beim Beenden werden sie verworfen; die Dateien auf der Festplatte bleiben unverändert.
unsaved-discard-and-quit = Änderungen verwerfen und beenden

## Import check dialog

dialog-import-check-title = Importprüfung
import-check-offender = { $name }: { $verdict }
import-check-offender-note = { $name }: { $verdict } – { $note }
import-check-file =
    { $count ->
        [one] { $count } Textur
       *[other] { $count } Texturen
    }: { $detail }
import-check-more =
    { $count ->
        [one] …und { $count } weitere markierte Datei.
       *[other] …und { $count } weitere markierte Dateien.
    }
import-check-summary =
    Entscheidung erforderlich für { $target }: { $flagged } von { $total ->
        [one] { $total } Datei
       *[other] { $total } Dateien
    } – { $incompatible ->
        [one] { $incompatible } inkompatible Textur
       *[other] { $incompatible } inkompatible Texturen
    }, { $unknown } unbekannt.
import-check-note = Importe werden in jedem Fall unverändert übernommen – das Format ist nur wichtig, wenn das Spiel diese Texturen laden muss.
import-check-import-anyway = Trotzdem importieren
import-check-cancel = Import abbrechen

## Import folder dialog

dialog-folder-import-title = Ordner importieren
folder-import-path = Ordner: { $path }
folder-import-summary =
    { $count ->
        [one] { $count } reguläre Datei
       *[other] { $count } reguläre Dateien
    } • { $size }
folder-import-top-level = Es werden nur Dateien direkt in diesem Ordner einbezogen; Unterordner werden nicht durchsucht.
folder-import-no-duplicates = Keine doppelten Namen gefunden.
folder-import-duplicates =
    { $count ->
        [one] { $count } doppelter Name gefunden. Wähle, wie damit umgegangen werden soll.
       *[other] { $count } doppelte Namen gefunden. Wähle, wie damit umgegangen werden soll.
    }
folder-import-skipped =
    { $count ->
        [one] { $count } Element konnte nicht geprüft werden und wird übersprungen.
       *[other] { $count } Elemente konnten nicht geprüft werden und werden übersprungen.
    }
folder-import-files = Dateien importieren
folder-import-skip-duplicates = Importieren (Duplikate überspringen)
folder-import-replace-duplicates = Duplikate ersetzen

## Update check dialog

dialog-update-title = Updateprüfung
update-available = Update verfügbar: { $version }
update-latest = Du verwendest die neueste Version.
# $error is a technical detail and stays in English.
update-failed = Updateprüfung fehlgeschlagen: { $error }
update-dont-show = Diese Nachricht nicht mehr anzeigen
update-open-releases = Releases öffnen

## Validate textures dialog

dialog-validator-title = Texturen validieren
validator-intro = Wähle das Spiel, für das dieses Archiv bestimmt ist. Der Validator markiert jede Textur außerhalb der im Original akzeptierten Formate dieser Engine und listet die Unbekannten auf, die plausibel laden könnten, aber nicht spielnativ sind.
validator-last-run = Letzter Lauf: { $counts }
validator-no-textures = keine Texturen
validator-current-target = aktuelles Ziel
validator-validate-for = Für { $game } validieren
validator-native-heading = Nativ (im Original verifiziert):
validator-unknown-heading = Unbekannt / nicht spielnativ:
validator-hint-likely = Inhalt sieht nach { $game } aus
validator-hint-possible = Inhalt möglicherweise { $game }
validator-hint-current = aktuelles Ziel: { $game }
validator-use-as-target = { $game } als Ziel verwenden
validator-already-target = (bereits das Ziel)
validator-highlight-rows = Zeilen hervorheben
legend-native = nativ
legend-native-description = Von dieser Engine erstellt – keine Aktion nötig.
legend-supported = unterstützt / konvertierbar
legend-supported-description = Lädt, ist aber nicht der Datendialekt des Spiels; ein verlustfreies Umschreiben kann angeboten werden.
legend-lossy = verlustbehaftete Konvertierung
legend-lossy-description = Erst nach einer pixelverändernden Konvertierung (Komprimierung oder Quantisierung) verwendbar.
legend-unknown = unbekannt
legend-unknown-description = Keine Hinweise in beide Richtungen – nicht als inkompatibel bekannt. Mit Vorsicht behandeln.
legend-incompatible = inkompatibel
legend-incompatible-description = Die ausgewählte Engine kann dieses Format nicht verarbeiten.

## Sort by dialog

dialog-sort-title = Sortieren nach — { $archive }
dialog-sort-title-no-archive = Sortieren nach — (kein Archiv geöffnet)
sort-empty = Noch keine Kriterien. Füge unten ein Kriterium hinzu, um zu sortieren.
sort-intro = Lege Prioritätsregeln fest und wende sie auf das aktuelle Archiv an.
sort-select-key = Kriterium auswählen…
sort-add-key = + Kriterium hinzufügen
sort-add-key-max = + Kriterium hinzufügen (Maximum erreicht)
sort-keys-active =
    { $active } von { $total ->
        [one] { $total } Kriterium aktiv
       *[other] { $total } Kriterien aktiv
    }
sort-preview-heading = Live-Vorschau (erste 10 Einträge)
sort-preview-empty = (keine Einträge im aktuellen Archiv)
sort-apply-preset = Voreinstellung anwenden…
sort-ascending-short = Auf ▲
sort-descending-short = Ab ▼
sort-key-name = Name
sort-key-extension = Erweiterung
sort-key-type = Typ
sort-key-size = Größe
sort-key-offset = Offset
sort-key-ide-file = IDE-Datei
sort-key-col-file = COL-Datei
sort-preset-name-az = Name (A→Z)
sort-preset-name-za = Name (Z→A)
sort-preset-type-then-name = Typ, dann Name
sort-preset-size-desc = Größe (groß → klein)
sort-preset-offset-asc = Offset (niedrig → hoch)

## Windows file dialogs (titles and file-type filters)

file-dialog-open-archive = IMG-Archiv öffnen
file-dialog-filter-img = IMG-Archiv
file-dialog-compare = Mit Eintragsliste vergleichen
file-dialog-filter-entry-list = IMG-Eintragsliste
file-dialog-open-agr = Bully-Animationsgruppe öffnen
file-dialog-filter-agr = Bully-Animationsgruppe
file-dialog-import-files = Dateien importieren
file-dialog-filter-importable = Importierbare Dateien
file-dialog-import-folder = Ordner zum Importieren auswählen
file-dialog-game-folder = Spielordner auswählen
file-dialog-choose-image = Bild auswählen
file-dialog-filter-images = Bilder
file-dialog-save-archive = IMG-Archiv speichern
file-dialog-export-list = Eintragsliste exportieren
file-dialog-export-folder = Exportordner auswählen

## Toasts: archives and selection

toast-no-archive-selected = Kein Archiv ausgewählt.
toast-archive-closed = Das Archiv ist nicht mehr geöffnet.
toast-target-archive-closed = Das Zielarchiv ist nicht mehr geöffnet.
toast-target-archive-deselected = Das Zielarchiv ist nicht mehr ausgewählt.
toast-validated-archive-closed = Das validierte Archiv wurde geschlossen.
toast-entry-unavailable = Der ausgewählte Eintrag ist nicht mehr verfügbar.
toast-task-running = Eine andere Aufgabe läuft noch.
toast-operation-running = Es läuft bereits eine Archivoperation.
toast-open-archive-to-validate = Öffne zuerst ein Archiv, um es zu validieren.
toast-open-archive-to-import = Öffne zuerst ein Archiv, um etwas hinein zu importieren.
toast-open-archive-to-import-folder = Öffne zuerst ein Archiv, um einen Ordner zu importieren.
toast-open-img-for-model = Öffne zuerst ein IMG-Archiv; das Modell stammt daraus.
toast-already-open = Bereits geöffnet: { $path }
# $error is a technical detail and stays in English.
toast-open-failed = Archiv konnte nicht geöffnet werden: { $error }
toast-file-gone = Datei existiert nicht mehr: { $path }
toast-file-not-found = Datei nicht gefunden: { $path }
toast-compare-empty-archive = Das ausgewählte Archiv hat keine Einträge zum Vergleichen.

## Toasts: saving and packing

toast-archive-saved = Archiv gespeichert.
toast-save-cancelled = Speichern abgebrochen.
toast-save-failed = Speichern fehlgeschlagen: { $error }
toast-save-before-pack = Speichere das Archiv, bevor du es packst.
toast-packed = Archiv gepackt – { $reclaimed } freigegeben ({ $size } auf der Festplatte).
toast-packed-nothing = Archiv gepackt – kein Speicherplatz freigegeben ({ $size } auf der Festplatte).
toast-pack-failed = Packen fehlgeschlagen: { $error }
toast-headers-repaired =
    { $count ->
        [one] { $count } Texturheader repariert; wird gespeichert.
       *[other] { $count } Texturheader repariert; wird gespeichert.
    }

## Toasts: game folder

toast-game-folder-set = Spielordner für { $archive }: { $path }
toast-game-folder-automatic = Spielordner für { $archive }: { $path } (automatisch)
toast-game-folder-none = { $archive } hat keinen Spielordner.
toast-game-folder-needs-save = Speichere das Archiv zuerst; der Spielordner wird pro Archivdatei gespeichert.

## Toasts: 3D viewer and animation

toast-select-model = Wähle zuerst einen NIF-, DFF- oder COL-Eintrag aus.
toast-viewer-unsupported = Der integrierte 3D-Viewer unterstützt .nif, .dff und .col ({ $entry }).
toast-select-ifp = Wähle zuerst einen .ifp-Eintrag aus.
toast-no-model-for-agr = Kein passendes .nif-Modell für { $animation } in diesem Archiv gefunden.
toast-no-model-for-agr-open = Kein passendes .nif-Modell für { $animation } im geöffneten Archiv gefunden.
toast-loading-animation = { $animation } wird auf { $model } geladen…
toast-loading-animation-hxd = { $animation } wird auf { $model } geladen… (HXD-Katalog gefunden)
toast-playback-stalled = Wiedergabe nach längerem Stocken pausiert.
toast-demo-loaded = Synthetische Animationsdemo geladen (keine Spieldaten).
toast-demo-closed = Animationsdemo geschlossen.
toast-animation-ready = Animation bereit: { $summary }
toast-animation-failed = Animation konnte nicht geladen werden: { $error }
toast-3d-load-failed = 3D-Laden fehlgeschlagen: { $error }
anim-summary-ifp =
    { $animation } auf { $model } ({ $clips ->
        [one] { $clips } Clip
       *[other] { $clips } Clips
    }, GTA IFP)
anim-summary-agr =
    { $animation } auf { $model } ({ $clips ->
        [one] { $clips } Clip
       *[other] { $clips } Clips
    })
anim-summary-agr-named =
    { $animation } auf { $model } ({ $clips ->
        [one] { $clips } Clip
       *[other] { $clips } Clips
    }, HXD-benannt)

## Toasts: textures and conversion

toast-select-texture = Wähle zuerst einen Textur-Eintrag aus.
toast-no-target = Kein Ziel für dieses Archiv festgelegt. Wähle das Spiel unter „Texturen validieren“.
toast-target-error = { $error } Wähle das Spiel dieses Archivs unter „Texturen validieren“.
toast-replace-busy = Es wird bereits ein Ersatz vorbereitet.
toast-preparing-replacement = Ersatz wird vorbereitet…
toast-replace-failed = Ersetzen fehlgeschlagen: { $error }
toast-import-busy = Es wird bereits ein Import vorbereitet.
toast-preparing-import = Import wird vorbereitet…
toast-txd-name-required = Gib dem neuen TXD einen Namen.
toast-entry-exists = Ein Eintrag namens „{ $name }“ existiert bereits.
toast-entry-added = „{ $name }“ hinzugefügt – speichere das Archiv, um es zu schreiben.
toast-set-target-first = Lege zuerst ein Spielziel fest (Texturen validieren).
toast-select-entries-to-convert = Wähle zuerst die zu konvertierenden Einträge aus.
toast-archive-changed-planning = Das Archiv hat sich während der Konvertierungsplanung geändert; bitte erneut versuchen.
toast-convert-only-txd = Nur TXD-Einträge können konvertiert werden – keiner der ausgewählten Einträge ist ein TXD-Texturcontainer.
toast-convert-all-native = Alle ausgewählten Texturen sind bereits nativ für das Ziel.
toast-archive-changed-after-plan = Das Archiv hat sich geändert, nachdem diese Konvertierung geplant wurde; bitte erneut versuchen.
toast-archive-changed-during = Das Archiv hat sich während der Konvertierung geändert; veraltete Ergebnisse wurden verworfen.
toast-converted =
    { $count ->
        [one] { $count } Eintrag konvertiert – speichere das Archiv, um ihn zu schreiben.
       *[other] { $count } Einträge konvertiert – speichere das Archiv, um sie zu schreiben.
    }
toast-conversion-failed = Konvertierung fehlgeschlagen: { $error }
toast-no-decoded-textures = Keine dekodierten Texturen zum Exportieren.
toast-decoded =
    { $count ->
        [one] { $count } Textur dekodiert
       *[other] { $count } Texturen dekodiert
    }
toast-decoded-not-retained =
    { $count ->
        [one] { $count } Textur dekodiert, aber die Vorschau konnte nicht behalten werden
       *[other] { $count } Texturen dekodiert, aber die Vorschau konnte nicht behalten werden
    }
toast-pick-embedded-folder = Wähle einen Ordner zum Exportieren der eingebetteten Texturen aus { $model }
toast-basename-unknown = Der Basisname von { $entry } kann nicht ermittelt werden
toast-read-failed = { $name } konnte nicht gelesen werden: { $error }

## Toasts: import and export

toast-import-cancelled = Import abgebrochen.
toast-imported =
    { $count ->
        [one] { $count } Datei importiert.
       *[other] { $count } Dateien importiert.
    }
toast-imported-unchecked =
    { $count ->
        [one] { $count } Datei importiert – kein Validator-Ziel festgelegt, Formate wurden nicht geprüft.
       *[other] { $count } Dateien importiert – kein Validator-Ziel festgelegt, Formate wurden nicht geprüft.
    }
toast-import-failed = Import fehlgeschlagen: { $error }
toast-no-files-in-folder = Keine regulären Dateien in { $folder } gefunden.
toast-folder-scan-failed = Ordnerprüfung fehlgeschlagen: { $error }
toast-folder-import-failed = Ordnerimport fehlgeschlagen: { $error }
toast-folder-import-done = Ordnerimport abgeschlossen: { $imported } importiert, { $skipped } übersprungen, { $failed } fehlgeschlagen.
toast-folder-import-cancelled = Ordnerimport abgebrochen: { $imported } importiert, { $skipped } übersprungen, { $failed } fehlgeschlagen.
toast-see-log = Details stehen im Archivprotokoll.
toast-exported =
    { $count ->
        [one] { $count } Eintrag exportiert.
       *[other] { $count } Einträge exportiert.
    }
toast-export-failed = Export fehlgeschlagen: { $error }
toast-entry-list-exported =
    { $count ->
        [one] { $count } Eintragsname nach { $path } exportiert.
       *[other] { $count } Eintragsnamen nach { $path } exportiert.
    }
toast-entry-list-export-failed = Eintragslisten-Export fehlgeschlagen: { $error }
toast-compare-failed = Eintragslisten-Vergleich fehlgeschlagen: { $error }
toast-no-missing-to-copy = Es gibt keine fehlenden Einträge zum Kopieren.
toast-copied-missing =
    { $count ->
        [one] { $count } fehlender Eintragsname kopiert.
       *[other] { $count } fehlende Eintragsnamen kopiert.
    }

## Toasts: clipboard, dragging and other actions

toast-copied-entry-details = Details des ausgewählten Eintrags kopiert.
toast-copied-logs = Protokoll kopiert.
toast-copied-name = Name kopiert: { $name }
toast-drag-cancelled = Ziehen abgebrochen.
toast-moved-entries =
    { $count ->
        [one] { $count } Eintrag nach Archiv #{ $archive } verschoben.
       *[other] { $count } Einträge nach Archiv #{ $archive } verschoben.
    }
toast-autoscroll = Autoscroll aktiv: Bewege den Zeiger zum Scrollen. Klick, Mittelklick, Rechtsklick, Mausrad oder eine Taste beenden den Modus.
toast-association-added = IMG Editor Plus steht jetzt im Datei-Explorer unter „Öffnen mit“ für .img/.dir. Um es als Standard festzulegen, wähle es auf der geöffneten Einstellungsseite aus.
toast-association-removed = Die .img/.dir-Zuordnung wurde entfernt.
toast-association-failed = Dateizuordnung fehlgeschlagen: { $error }
toast-validation-cancelled = Validierung abgebrochen.
toast-validation-failed = Validierung fehlgeschlagen: { $error }
toast-no-compat-issues = Keine Kompatibilitätsprobleme gefunden – { $summary }
# $verdicts is the per-verdict count list, e.g. "native 12, untested 1".
validation-summary =
    Für { $game } validiert: { $txds ->
        [one] { $txds } TXD
       *[other] { $txds } TXDs
    } ({ $textures ->
        [one] { $textures } Textur
       *[other] { $textures } Texturen
    }): { $verdicts }; { $errors } Fehler, { $warnings ->
        [one] { $warnings } Warnung
       *[other] { $warnings } Warnungen
    }

## Archive log (the Logs box in the Export tab)

log-viewer-ready = Integrierter 3D-Viewer bereit
log-viewer-ready-cached = Integrierter 3D-Viewer bereit (zwischengespeichert)
log-viewer-opened = 3D-Viewer geöffnet: { $name }
log-viewer-failed = 3D-Viewer fehlgeschlagen: { $reason }
log-viewer-closed = 3D-Viewer geschlossen
log-external-viewer = Externer 3D-Viewer wird für { $name } geöffnet
log-exported =
    { $count ->
        [one] { $count } Eintrag exportiert
       *[other] { $count } Einträge exportiert
    }
log-export-failed = Export fehlgeschlagen: { $error }
log-entry-list-exported =
    { $count ->
        [one] Eintragsliste ({ $count } Name) nach { $path } exportiert
       *[other] Eintragsliste ({ $count } Namen) nach { $path } exportiert
    }
log-compat-check = Kompatibilitätsprüfung: { $summary }
log-decoded =
    { $count ->
        [one] { $count } Texturvorschau dekodiert
       *[other] { $count } Texturvorschauen dekodiert
    }
log-texture-export-failed = Export von { $model } fehlgeschlagen: { $error }
# "Exported <what>" in the Export tab's recent list.
recent-exported = Exportiert: { $what }
recent-exported-files =
    { $count ->
        [one] { $count } Datei
       *[other] { $count } Dateien
    }

## Empty workspace pro tips (they name menus and keys; keep those in step
## with the translated menu labels)

pro-tip-label = Tipp:
pro-tip-search = Drücke Ctrl+F, um die Suche zu fokussieren, und springe dann mit ↑/↓ und Enter zu einem Treffer.
pro-tip-search-context = Suchvorschläge zeigen einen Treffer in seinem Archivkontext; Ansicht → Auswahlkontext in der Suche aktiviert isolierte Ergebnisse.
pro-tip-context-menu = Rechtsklick auf einen Eintrag für 3D-Ansicht, Texturen, Export, Umbenennen und weitere Aktionen.
pro-tip-autoscroll = Mittelklick auf die Eintragsliste für Autoscroll im Browser-Stil; optionale Trägheit findest du unter Ansicht.
pro-tip-tab-keys = Drücke 1, 2 oder 3, um zu Export, 3D-Ansicht oder Textur zu wechseln.
pro-tip-close-tab = Mittelklick auf einen Archiv-Tab, um ihn schnell zu schließen.
pro-tip-uv-overlay = Textur-UV-Overlays sind verfügbar, wenn das ausgewählte Modell passende Geometrie mitbringt.
pro-tip-wire-grid = In der 3D-Ansicht zeigt „Drahtgitter“ die Dreieckskanten, und das „Bodenraster“ hilft beim Einschätzen der Größe.
pro-tip-save-keys = Verwende Ctrl+S zum schnellen Speichern und Ctrl+Shift+S, um ein Archiv unter neuem Namen zu speichern.
pro-tip-unique-exports = Exportierte Texturen erhalten automatisch eindeutige Dateinamen, sodass Batch-Exporte sich nie gegenseitig überschreiben.
pro-tip-agr = Bearbeiten → .agr-Animationsdatei laden spielt eine Animation in der 3D-Ansicht ab; Leertaste schaltet die Wiedergabe um, ←/→ geht Frame für Frame.
pro-tip-model-picker = Während eine Animation läuft, spielt die Modellauswahl im Dock sie auf jedem kompatiblen Modell im Archiv erneut ab.
pro-tip-fullscreen-preview = Das Vergrößern-Symbol in einer Import- oder Ersetzen-Vorschau öffnet sie im Vollbild: Scrollen zum Zoomen, Ziehen zum Verschieben, Esc zum Schließen.
pro-tip-bulk-convert = „Auswahl in den Zieldialekt konvertieren“ kodiert die ausgewählten TXDs gesammelt neu – wähle zuerst das Spiel unter „Texturen validieren“.
pro-tip-entry-lists = Ctrl+L exportiert eine Eintragsliste und Ctrl+P vergleicht sie mit dem Archiv, um fehlende Namen zu finden.
toast-no-dff-to-animate = Kein DFF-Modell zum Animieren in diesem Archiv gefunden.
toast-replace-needs-txd = Ersetzen funktioniert mit TXD-Einträgen; dieser Eintrag ist keiner.
toast-texture-replaced = Textur ersetzt – speichere das Archiv, um sie zu schreiben.
toast-bully-texture-writing = Das Schreiben von Bully-Texturen (Gamebryo) wird noch nicht unterstützt.
toast-archive-file-missing = Die Archivdatei existiert nicht mehr. Verwende zuerst „Speichern unter…“.
toast-folder-scan-target-changed = Das Zielarchiv hat sich geändert, während der Ordner durchsucht wurde.
toast-folder-import-discarded = Ordnerimport verworfen, weil sich das Zielarchiv geändert hat.
toast-compare-discarded = Vergleich verworfen, weil sich das Archiv geändert hat; bitte erneut versuchen.
toast-drop-needs-archive = Öffne zuerst ein Archiv, um Nicht-IMG-Dateien hineinzuziehen.
toast-no-game-root = Der Spielordner konnte nicht aus dem Archivpfad ermittelt werden.
toast-textures-exported =
    { $count ->
        [one] { $count } Textur exportiert.
       *[other] { $count } Texturen exportiert.
    }
toast-textures-export-failed = Texturexport fehlgeschlagen: { $error }
error-write-file = { $path } konnte nicht geschrieben werden: { $error }
error-read-entry = Eintrag konnte nicht gelesen werden: { $error }
error-texture-decode = Texturdekodierung fehlgeschlagen: { $error }
error-texture-preview-unsupported = Die Texturvorschau unterstützt TXD- und NFT-Einträge; „{ $entry }“ ist kein unterstützter Texturcontainer.

## Entry table and search

table-name = Name
table-size = Größe
# Size column for entries still read from the archive (sectors × 2 KB).
table-size-kb = { $size } KB
table-no-matches = Keine Einträge entsprechen dem aktuellen Filter.
search-label = Suchen:
search-did-you-mean = Meintest du:
sort-tip-name-asc = Nach Dateiname sortiert (A → Z).
sort-tip-name-desc = Nach Dateiname sortiert (Z → A).
sort-tip-name-inactive = Nach Dateiname sortieren (A → Z).
sort-tip-type-primary = Nach Dateityp sortiert, { $type } zuerst.
sort-tip-type-alphabetical = Nach Dateityp alphabetisch sortiert.
sort-tip-type-inactive = Nach Dateityp sortieren (alphabetisch).
sort-tip-size-desc = Nach Größe sortiert (größte zuerst).
sort-tip-size-asc = Nach Größe sortiert (kleinste zuerst).
sort-tip-size-inactive = Nach Größe sortieren (größte zuerst).
version-unknown = Unbekannt

## Toolbar tooltips

toolbar-new = Neu
toolbar-open = Öffnen
toolbar-save = Speichern
toolbar-pack = Archiv packen
toolbar-import = Importieren
toolbar-import-folder = Ordner importieren
toolbar-export-selected = Auswahl exportieren
toolbar-delete-selected = Auswahl löschen
toolbar-validate = Texturen validieren
toolbar-image-as-txd = Bild als TXD importieren
toolbar-convert-selection = Auswahl in den Zieldialekt konvertieren

## Empty workspace and status bar

empty-heading = Öffne oder erstelle ein Archiv, um zu beginnen.
empty-drop-hint = Oder ziehe eine .img- oder .dir-Datei hierher, um sie zu öffnen.
status-selected = Ausgewählt: { $count }

## Inspector tabs

tab-export = Export
tab-3d-view = 3D-Ansicht
tab-texture = Textur

## Export tab

export-format = Format
export-entries = Einträge
export-entries-value = { $total } (sichtbar: { $visible })
export-game-folder = Spielordner
export-game-folder-automatic = { $path } (automatisch)
export-game-folder-none = keiner
export-game-folder-unsaved = keiner (nicht gespeichertes Archiv)
export-progress = Fortschritt
export-ready = Bereit zum Exportieren
export-open-folder = Exportordner öffnen
export-selected-entry = Ausgewählter Eintrag:
export-logs = Protokoll:
export-recent = Letzte Exporte:
button-copy = Kopieren

## Entry details (Export tab, and the Copy button's clipboard text)

inspect-name = Name
inspect-type = Typ
inspect-size = Größe
inspect-offset = Offset
inspect-source = Quelle
inspect-size-mb = { $mb } MB ({ $bytes } Bytes, { $sectors } Sektoren)
inspect-size-kb = { $kb } KB ({ $bytes } Bytes, { $sectors } Sektoren)
inspect-size-bytes = { $bytes } Bytes ({ $sectors } Sektoren)
inspect-offset-value = Sektor { $sector } (Byte { $byte })
inspect-hex-preview = Vorschau (Hex):

## 3D view tab

viewer-no-archive = Kein Archiv geöffnet.
viewer-try-demo = Synthetische Animationsdemo ausprobieren
viewer-select-model = Wähle einen .nif-, .dff- oder .col-Eintrag, um ihn in 3D anzuzeigen.
viewer-gpu-unavailable = GPU-Viewer nicht verfügbar
viewer-gpu-hint = Versuche, die Vorschau zu leeren oder ein kleineres Modell auszuwählen.
viewer-clear-error = Viewerfehler zurücksetzen
viewer-selected-model = ausgewähltes Modell
viewer-preparing = 3D-Vorschau wird vorbereitet
viewer-preparing-detail = Geometrie wird gelesen und Texturen werden aufgelöst…
viewer-preparing-cache-note = Zukünftige Vorschauen dieses Modells sind sofort verfügbar.
viewer-ready = Bereit, dieses Modell in 3D anzuzeigen.
viewer-unsupported-entry = Der integrierte Viewer rendert .nif-, .dff- und .col-Einträge. { $entry } ist kein unterstütztes Modell — verwende das Rechtsklickmenü für einen anderen Viewer.
viewer-load-selected-hint = Verwende oben „{ viewer-load-selected }“, um dieses Modell anzuzeigen.
viewer-right-click-hint = Wähle einen .nif-, .dff- oder .col-Eintrag und klicke mit der rechten Maustaste → { context-open-3d }.
viewer-toolbar-label = 3D:
viewer-preparing-selected = Ausgewähltes Modell wird vorbereitet…
viewer-load-selected = Auswahl laden
viewer-load-selected-tip = Das ausgewählte Modell in den 3D-Viewer laden.
viewer-reset = Ansicht zurücksetzen
viewer-reset-tip = Die Kamera wieder auf das Modell ausrichten. Tastenkürzel: R
viewer-clear = Leeren
viewer-clear-tip = Die geladene Szene verwerfen
viewer-wireframe = Drahtgitter
viewer-wireframe-tip = Dreieckskanten über dem schattierten Modell anzeigen.
viewer-cull = Rückseiten ausblenden
viewer-cull-tip = Rückseitige Dreiecke ausblenden, um die Oberflächenorientierung zu prüfen.
viewer-textured = Texturiert
viewer-textured-tip = Die dekodierten Texturen des Modells statt eines neutralen Materials verwenden.
viewer-alpha = Alpha-Blending
viewer-alpha-tip = Textur-Alpha für Aussparungen und transparente Materialien berücksichtigen.
viewer-alpha-unavailable-tip = Aktiviere { viewer-textured } bei einem Modell mit Texturen, um Alpha-Blending zu verwenden.
viewer-center = Ursprung zentrieren
viewer-center-tip = Das Modell zur Prüfung neu zentrieren; deaktivieren, um Weltkoordinaten beizubehalten.
viewer-grid = Bodenraster
viewer-grid-tip = Das Weltreferenzraster und die XYZ-Achsen anzeigen.
viewer-stats =
    { $vertices ->
        [one] { $vertices } Vertex
       *[other] { $vertices } Vertices
    }   { $triangles ->
        [one] { $triangles } Dreieck
       *[other] { $triangles } Dreiecke
    }   { $textures ->
        [one] { $textures } Textur
       *[other] { $textures } Texturen
    }   { $width }×{ $height }   { $orientation }   { $origin }
viewer-origin-centered = zentriert
viewer-origin-world = Welt
viewer-preparing-entry = { $entry } wird vorbereitet…
viewer-no-scene = Keine Szene geladen

## Animation dock

anim-preparing = Animation wird vorbereitet
anim-preparing-detail = Clips werden dekodiert und Texturen aufgelöst…
anim-preparing-cache-note = Zukünftige Wiedergaben dieses Paares sind sofort verfügbar.
anim-preparing-label = { $label } wird vorbereitet…
anim-title = Animation
anim-title-demo = Animationsdemo
anim-demo-note = synthetische Fixtures — keine Spieldaten
anim-exit-demo = Demo beenden
anim-loop = Wiedergabe wiederholen
anim-speed = Geschwindigkeit
anim-pack = Animationspaket
anim-model-tip = Diese Animation auf einem anderen Modell abspielen
anim-clip = Clip
anim-play = Abspielen (Leertaste)
anim-pause = Pausieren (Leertaste)
anim-jump-start = Zum Anfang springen (Pos1)
anim-step-back = Ein Frame zurück (←)
anim-step-forward = Ein Frame vor (→)
anim-jump-end = Zum Ende springen (Ende)
anim-stop = Stopp
anim-rate-source = Quelle mit { $fps } fps
anim-rate-preview = Vorschau mit { $fps } fps
anim-frame = Einpassen
anim-frame-rest = Ruhepose
anim-frame-rest-tip = Die Ruhepose einpassen
anim-frame-pose = Pose
anim-frame-pose-tip = Die aktuelle Pose einpassen
anim-frame-motion = Bewegung
anim-frame-motion-tip = Die gesamte Bewegung einpassen
anim-in-place-tip = Root an Ort und Stelle — Root-Translation verwerfen
anim-follow-tip = Der Root-Bewegung mit der Kamera folgen
anim-skeleton-tip = Die Skelett-Überlagerung anzeigen
anim-motion-path-tip = Den Pfad der Root-Bewegung anzeigen
anim-ground-tip = Den tiefsten Punkt des Clips auf den Boden setzen
anim-crossfade-tip = Beim Clipwechsel überblenden
anim-keys-hint = Leertaste Abspielen/Pause · ←/→ Frame-Schritte · Pos1/Ende Bereichsenden · Ziehen zum Spulen

## Texture tab

texture-no-archive = Kein Archiv geöffnet.
texture-select-entry = Wähle einen TXD-, NFT-, NIF- oder DFF-Eintrag, um Texturen anzuzeigen.
texture-not-container = { $entry } ist kein Texturcontainer. Eine Vorschau ist für TXD-, NFT- oder gerenderte Modell-Einträge verfügbar.
texture-no-companions = Für { $entry } wurden keine zugehörigen Texturen aufgelöst.
texture-load-model-hint = Lade das ausgewählte Modell, um seine Texturen aufzulösen.
texture-load-model = Ausgewähltes Modell laden
texture-not-decoded = { $kind } { $entry } ist noch nicht dekodiert.
texture-load-textures = Texturen aus ausgewähltem { $kind } laden
texture-none-decodable = Keine dekodierbaren Texturen in diesem Container.
texture-animation-model = Animationsmodell { $entry } — wechsle das Modell im 3D-Dock
texture-export =
    { $count ->
        [one] Textur exportieren ({ $count })
       *[other] Texturen exportieren ({ $count })
    }
texture-slot = Textur { $index }/{ $count }
texture-alpha = Alpha:
texture-yes = Ja
texture-no = Nein
texture-verdict-default = Bewertung für ein { $game }-Ziel.
texture-pal8-ready = PAL8-bereit ({ $colors } Farben)
texture-pal8-tip = Jedes Pixel ist eine dieser unterschiedlichen Farben, daher speichert eine 8-Bit-Palette diese Textur ohne Quantisierung.
texture-replace = Textur ersetzen…
texture-replace-hint = Importiert PNG/DDS/BMP/TGA und kodiert für das Ziel des Archivs neu.
texture-uv-standalone-tip = Vorschau nur der Textur. Wähle ein passendes DFF- oder NIF-Modell, um UV-Mapping zu aktivieren.
texture-uv-unavailable-tip = UV-Mapping ist verfügbar, nachdem passende Modellgeometrie geladen wurde.
texture-uv-tip = Die UV-Dreiecke der passenden Modellgeometrie anzeigen.
texture-uv = UV-Karte anzeigen
texture-uv-standalone = Nur-Textur-Vorschau · UV-Karte benötigt passende Modellgeometrie
texture-uv-unavailable = Passende Modellgeometrie laden, um UVs zu aktivieren
texture-uv-triangles =
    { $count ->
        [one] { $count } Dreieck
       *[other] { $count } Dreiecke
    }
texture-only-badge = Nur-Textur-Vorschau
texture-grid = Raster
texture-grid-tip = Ein Referenzraster über der Texturvorschau anzeigen.
texture-grid-size-tip = Ein { $size }×{ $size }-Referenzraster für die Textur verwenden.
texture-grid-size = Größe:
texture-fullscreen-tip = Im Vollbild anzeigen (volle Qualität)
texture-model-companion = Zugehörige Modelltextur

## Entry table: the Type column and curated file-type names
## (the English names double as sort and grouping keys; only the
## displayed text is translated)

table-type = Typ
# $arrow is ↑ or ↓; $primary is the file type sorted first.
table-type-sorted = Typ { $arrow } { $primary }
file-type-model = Modell
file-type-texture = Textur
file-type-collision = Kollision
file-type-animation = Animation
file-type-placement = Platzierung
file-type-definition = Definition
file-type-data = Daten
archive-untitled = Unbenannt

## Archive log (Export tab)

log-archive-created = Archiv erstellt
log-archive-saved = Archiv gespeichert
log-archive-packed =
    { $entries ->
        [one] Archiv gepackt: { $entries } Eintrag, { $reclaimed } freigegeben
       *[other] Archiv gepackt: { $entries } Einträge, { $reclaimed } freigegeben
    }
log-imported-entries =
    { $count ->
        [one] { $count } Eintrag importiert
       *[other] { $count } Einträge importiert
    }
log-folder-import = Ordnerimport: { $imported } importiert, { $skipped } übersprungen, { $failed } fehlgeschlagen
log-folder-import-detail = Details zum Ordnerimport: { $detail }
folder-import-cancelled-detail = Import abgebrochen; die restlichen Dateien wurden übersprungen.
folder-import-duplicate = { $name }: Duplikat übersprungen

## Entry details: source and format summary

inspect-source-imported = Importiert
inspect-source-imported-from = Importiert aus { $path }
inspect-source-archive = Archiv { $archive }, Sektor { $sector }
inspect-key-format = Format
inspect-key-version = Version
inspect-key-clump-size = Clump-Größe
inspect-key-endian = Endian
inspect-key-user-version = Benutzerversion
inspect-key-lines = Zeilen
inspect-key-atomics = Atomics
inspect-key-textures = Texturen
inspect-key-entries = Einträge
inspect-key-mesh-vertices = Mesh-Vertices
inspect-key-mesh-faces = Mesh-Flächen
inspect-key-spheres = Kugeln
inspect-key-boxes = Boxen
inspect-key-shadow-mesh = Schatten-Mesh
inspect-rw-truncated = RenderWare (abgeschnitten)
inspect-rw-clump = Clump (Modell)
inspect-rw-txd = Texturwörterbuch
inspect-rw-pi-txd = Plattformunabhängiges Texturwörterbuch
inspect-rw-animation = Animation
inspect-rw-uv-animation = UV-Animation
inspect-rw-stream = RenderWare-Stream
inspect-bytes =
    { $count ->
        [one] { $count } Byte
       *[other] { $count } Bytes
    }
inspect-col-unknown = Unbekannte Kollision
inspect-endian-big = Big
inspect-endian-little = Little
inspect-nif-truncated = NIF (abgeschnitten)
inspect-lines-value = { $lines } ({ $nonempty } nicht leer)
inspect-format-scm = GTA-Skript (main.scm)
inspect-format-ipl = GTA-Objektplatzierung
inspect-format-ide = GTA-Objektdefinition
inspect-texture-count =
    { $count ->
        [one] { $count } Textur
       *[other] { $count } Texturen
    }

## Embedded-texture export (NIF → NFT)

texture-export-none = Keine eingebetteten Texturen gefunden
texture-export-done =
    { $count ->
        [one] { $count } eingebettete Textur exportiert
       *[other] { $count } eingebettete Texturen exportiert
    }
texture-export-partial = Exportierte Texturen: { $written }, Fehler: { $failures }
texture-export-no-nft = Kein NFT für „{ $name }“ gefunden

## Manifest comparison errors

compare-manifest-inspect = Manifest „{ $path }“ konnte nicht geprüft werden: { $error }
compare-manifest-too-large = Manifest „{ $path }“ ist zu groß ({ $size }; Limit ist { $limit }).
compare-manifest-read = Manifest „{ $path }“ konnte nicht gelesen werden: { $error }
compare-manifest-grew = Manifest „{ $path }“ hat während des Lesens das Limit von { $limit } überschritten.
compare-manifest-utf8 = Manifest „{ $path }“ ist kein gültiges UTF-8: { $error }

## Compatibility verdicts (lowercase: they appear mid-sentence)

verdict-native = nativ
verdict-supported = unterstützt
verdict-convertible-lossless = konvertierbar (verlustfrei)
verdict-convertible-lossy = konvertierbar (verlustbehaftet)
verdict-unsupported = nicht unterstützt
verdict-untested = ungeprüft

## Compatibility evidence notes
## These cite measurements of the retail games. Keep format names
## (DXT1, PAL8, X8R8G8B8, D3D8, pp=1 …) and file names untranslated.
## "Raster" is one texture image inside a TXD; "dialect" is the set of
## texture formats a game's own files use.

compat-class-other-nif = andere NiPixelData-Formate
compat-cat-iii-pal = 96,5 % der Welttexturen des Originals
compat-cat-iii-888 = 6.806 Raster, inkl. Spieler-/Fahrzeugset
compat-cat-iii-8888 = 1.121 Raster
compat-cat-iii-1555 = 24 Raster
compat-cat-iii-dxt1 = Original enthält keine; D3D8-Hardware unterstützt es
compat-cat-iii-dxt = D3D8-Kompressionswerte 1-5; Original enthält keine
compat-cat-depth24 = dokumentierte 24-Bit-Tiefenform; Stride/Reihenfolge ungeprüft
compat-cat-iii-565 = treiberzugeordnet; III enthält keine
compat-cat-555-lum8 = Treiber ordnet C555 und LUM8 zu; Original enthält keine
compat-cat-a8l8 = D3D9/Decoder unterstützen es; RW-Nibble-Pfad ungeprüft
compat-cat-vc-dxt1 = Welt-Dialekt des Originals; D3D8 pp=1 (über 10.000 Raster, veraltete Nibbles)
compat-cat-vc-dxt3 = Alpha-Dialekt des Originals; D3D8 pp=3 (1.149 Raster)
compat-cat-vc-pal = 27 Raster; akzeptiert, aber selten
compat-cat-vc-888 = 1 Raster
compat-cat-vc-8888 = III enthält es; VC selbst enthält keine
compat-cat-vc-565 = die 565-Labels des Originals sind DXT1-Daten; kein echtes R565 gemessen
compat-cat-vc-16bit = die Labels des Originals sind DXT1/DXT3-Daten; rohe 16-Bit-Formen ungemessen
compat-cat-vc-dxt = D3D8-Kompressionswerte existieren; Original enthält nur 1 und 3
compat-cat-sa-dxt1 = 28.807 Raster über die vier Archive
compat-cat-sa-dxt3 = 2.098 Raster
compat-cat-sa-888 = 1.015 Raster, meist player.img-Skins
compat-cat-sa-8888 = 237 Raster
compat-cat-sa-dxt5 = Original enthält keine; D3D9 unterstützt es
compat-cat-sa-dxt24 = das D3D9-Formatwort trägt sie; prämultipliziertes Alpha
compat-cat-sa-pal = Quellen widersprechen sich; Original enthält keine; parsen und erhalten
compat-cat-sa-16bit = treiberzugeordnet; Original enthält kein unkomprimiertes 16-Bit
compat-cat-sa-a8l8 = Magic.TXD listet es für SA PC; RW-Pfad ungeprüft
compat-cat-bully-dxt1 = 31.714 Raster
compat-cat-bully-dxt5 = 3.526 Raster
compat-cat-bully-rgb = 138 / 134 Raster
compat-cat-bully-pal = 127 / 1 Raster
compat-cat-bully-dxt3 = Gamebryo unterstützt es; Original enthält keine
compat-cat-bully-other = 15 Raster nicht dekodierbar

compat-note-bully-not-rw = Bully-Assets sind Gamebryo NIF/NFT, keine RenderWare-Natives
compat-note-platform-rewrite = ein Plattform-{ $platform }-Raster in einem { $game }-Archiv muss für Plattform/Version umgeschrieben werden
compat-note-no-profile = noch keine Profiltabelle
compat-note-nft-not-rw = Gamebryo-NFT-Raster sind keine RenderWare-Natives
compat-note-bully-dxt1 = Original-Bully: 31.714 DXT1-Raster
compat-note-bully-dxt5 = Original-Bully: 3.526 DXT5-Raster
compat-note-bully-rgb = Original-Bully enthält rohes RGB/RGBA (138/134)
compat-note-bully-pal = Original-Bully enthält palettierte Raster (127 PAL + 1 PALA)
compat-note-bully-dxt3 = Gamebryo unterstützt DXT3, aber Original-Bully enthält keine
compat-note-sa-dxt = Original-SA: 28.807 DXT1 + 2.098 DXT3 Raster
compat-note-sa-dxt24 = das native D3D9-Format trägt DXT2/DXT4 (prämultipliziert); Original-SA enthält keine
compat-note-sa-dxt5 = Original-SA enthält keine; DXT5 lebt von der D3D9-Unterstützung (Mod-Tools nutzen es)
compat-note-sa-pal = Quellen widersprechen sich bei SA-Paletten; Original-SA enthält keine; parsen und erhalten
compat-note-sa-depth24 = dokumentierte 24-Bit-R8G8B8-Form; Original-SA enthält keine; Laufzeit ungeprüft
compat-note-sa-16bit = Treiber ordnet 1555/565/4444 zu; Original-SA enthält kein unkomprimiertes 16-Bit
compat-note-c555 = Treiber ordnet C555 X1R5G5B5 zu; Original enthält keine
compat-note-lum8 = Treiber ordnet LUM8 D3DFMT_L8 zu; Original enthält keine
compat-note-sa-a8l8 = D3D9 trägt A8L8 und unabhängige Decoder unterstützen es; gewöhnliches RW-Nibble-Mapping ungeprüft
compat-note-iii-pal = Original-III: 96,5 % PAL8; Original-VC: 27 Raster – akzeptiert, aber selten
compat-note-vc-dxt1 = Welt-Dialekt des Original-VC: DXT1 mit D3D8 pp=1; das Raster-Nibble ist veraltet
compat-note-iii-dxt1 = Original-III enthält keine komprimierten Raster (0/15.372); D3D8-Hardware unterstützt DXT1
compat-note-vc-dxt3 = Alpha-Dialekt des Original-VC: DXT3 mit D3D8 pp=3 (1.149 Raster)
compat-note-iii-dxt = D3D8-Kompressionswerte 1-5 entsprechen DXT1-5 (DXT2/4 prämultipliziert); Original-III+VC enthalten nur 1 und 3
compat-note-iii-depth24 = dokumentierte 24-Bit-R8G8B8-Form; Stride/Reihenfolge/Laufzeit ungeprüft
compat-note-iii-8888 = Original-III enthält 8888 in beiden Archiven (1.121 Raster)
compat-note-vc-565 = die als 565 beschrifteten Raster des Original-VC sind DXT1-Daten; kein echtes R565 gemessen
compat-note-iii-565 = treiberzugeordnete 16-Bit-Form; III enthält keine
compat-note-vc-1555 = die 1555-Labels des Original-VC sind DXT1-Daten; kein echtes R1555 gemessen
compat-note-vc-4444 = die 4444-Labels des Original-VC sind DXT3-Daten; kein echtes R4444 gemessen
compat-note-iii-4444 = III enthält keine; 16-Bit mit Alpha aus der D3D8-Ära
compat-note-iii-a8l8 = Magic.TXD listet A8L8 für SA PC; D3D9 trägt es; gewöhnlicher RW-Pfad ungeprüft

## Output-format choices (import and replace dialogs)

compat-choice-iii-888 = 32-Bit X8R8G8B8, verlustfrei; Standard im txd.img des Originals
compat-choice-iii-8888 = A8R8G8B8, behält Alpha; Original-III enthält 1.121
compat-choice-iii-pal8 = 8-Bit-Palette, quantisiert Farben; Welt-Dialekt des Originals (96,5 %)
compat-choice-pal4 = 4-Bit-Palette, quantisiert stark; nur 16-Farben-Artworks
compat-choice-iii-1555 = 16-Bit mit 1-Bit-Alpha; Original enthält 24
compat-choice-iii-dxt = hardwareunterstützt, aber von III nicht verwendet; verlustbehaftet
compat-choice-vc-dxt1 = Welt-Dialekt des Original-VC (D3D8 pp=1); verlustbehaftete Komprimierung
compat-choice-vc-dxt3 = Alpha-Dialekt des Original-VC (D3D8 pp=3); verlustbehaftete Komprimierung
compat-choice-vc-888 = 32-Bit X8R8G8B8, verlustfrei; Original-VC enthält eines
compat-choice-vc-8888 = A8R8G8B8, behält Alpha; Form aus der D3D8-Ära
compat-choice-vc-pal8 = 8-Bit-Palette, quantisiert Farben; Original-VC enthält 27
compat-choice-vc-565 = 16-Bit; Original-VC beschriftet 565 als DXT-Daten, rohe Form ungemessen
compat-choice-vc-4444 = 16-Bit mit Alpha; Original-VC beschriftet 4444 als DXT3-Daten
compat-choice-player-888 = 32-Bit X8R8G8B8; Original-player.img enthält 269
compat-choice-player-8888 = A8R8G8B8, behält Alpha; Original-player.img enthält 125
compat-choice-player-dxt = überall unterstützt; verlustbehaftet (player.img enthält keine)
compat-choice-sa-dxt1 = Welt-Dialekt des Original-SA; verlustbehaftete Komprimierung
compat-choice-sa-dxt3 = Alpha-Dialekt des Original-SA; verlustbehaftete Komprimierung
compat-choice-sa-8888 = A8R8G8B8, verlustfrei; Original-SA enthält es im player.img
compat-choice-sa-pal8 = quantisiert Farben; Original-SA enthält keine palettierten Raster

## Conversion warnings and errors

compat-warn-alpha-discarded = { $source } hat Alpha, aber { $format } kann es nicht speichern – der Alphakanal wird verworfen.
compat-warn-dxt-lossy = Die { $format }-Komprimierung ist verlustbehaftet; die Vorschau zeigt das kodierte Ergebnis.
compat-warn-palette-exact =
    { $colors ->
        [one] { $format } speichert das Bild exakt ({ $colors } Farbe).
       *[other] { $format } speichert das Bild exakt ({ $colors } Farben).
    }
compat-warn-palette-quantize = Die Farben werden auf höchstens { $cap } Einträge quantisiert.
compat-warn-stream-budget = { $width }x{ $height } überschreitet das 1024-px-Streaming-Budget von SA; das Spiel streamt es möglicherweise nicht.
# $verdict is one of the verdict-* labels above.
compat-warn-verdict = { $format } ist { $verdict } für { $game }: { $note }
compat-error-image-format = unbekanntes Bildformat: { $error }
compat-error-image-kind = { $kind }-Bilder werden nicht unterstützt; verwende PNG, DDS, BMP oder TGA
compat-error-decode = { $label }-Dekodierung fehlgeschlagen: { $error }
compat-error-image-size = nicht unterstützte Bildgröße { $width }x{ $height }
compat-error-unreadable-texture = „{ $name }“ kann nicht dekodiert werden ({ $error }); die Konvertierung braucht lesbare Pixel
compat-error-no-dimensions = Textur hat keine Abmessungen
compat-error-texture-index = Texturindex { $index } liegt außerhalb des Bereichs
compat-error-unknown-target = unbekanntes Spielziel „{ $id }“

## Save check: container conventions

compat-container-v2-expects-v1 = IMG-v2-Container, aber { $game } erwartet IMG v1 – das Spiel wird diese Dateien nicht sehen.
compat-container-v1-sa = IMG-v1-Container; Original-San Andreas verwendet IMG v2 (v1 lädt nur, wenn es in gta.dat gelistet ist).
compat-container-xbox = Xbox-360-Paketierung; { $game } erwartet einen PC-Container.

## Import check notes

compat-scan-unreadable = konnte nicht gelesen werden: { $error }
compat-scan-bad-nif = NIF-Header nicht lesbar: { $error }
compat-scan-empty-nif = keine NiPixelData-Blöcke (leerer Stub)
compat-scan-not-texture = keine TXD- oder Gamebryo-Textur

## Target-game suggestion evidence ($share is a percentage such as "92%")

compat-hint-gamebryo = { $total } Gamebryo-Einträge ({ $nft } NFT, { $nif } NIF)
compat-hint-d3d9 = Plattform 9 (D3D9) bei { $share } von { $total } geprüften Rastern
compat-hint-pal = PAL8/PAL4 bei { $share } der geprüften Raster
compat-hint-vc16 = 16-Bit 565/4444/1555 bei { $share } der geprüften Raster
compat-hint-d3d8 = Plattform 8 (D3D8)
