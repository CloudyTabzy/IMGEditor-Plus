### IMG Editor Plus - Español.
###
### Draft translation; pending review by a native speaker.
### Keep message ids and { $variables } exactly as in en.ftl.

## Menu bar: root menus

menu-file = Archivo
menu-recent = Recientes
menu-edit = Editar
menu-selection = Selección
menu-view = Ver
menu-themes = Temas
menu-help = Ayuda
menu-language = Idioma

## File menu

menu-file-new = Nuevo ({ $shortcut })
menu-file-open = Abrir… ({ $shortcut })
menu-file-save = Guardar ({ $shortcut })
menu-file-save-as = Guardar como… ({ $shortcut })
menu-file-pack = Compactar archivo
menu-file-set-game-folder = Elegir carpeta del juego…
menu-file-reset-game-folder = Restablecer carpeta del juego
menu-file-close-tab = Cerrar pestaña ({ $shortcut })
menu-file-sort-by = Ordenar por…

## Recent menu

menu-recent-empty = No hay archivos recientes

## Edit menu

menu-edit-import = Importar ({ $shortcut })
menu-edit-import-folder = Importar carpeta
menu-edit-export-all = Exportar todo ({ $shortcut })
menu-edit-export-selected = Exportar selección ({ $shortcut })
menu-edit-export-list = Exportar como lista ({ $shortcut })
menu-edit-compare-list = Comparar con lista ({ $shortcut })
menu-edit-load-agr = Cargar animación .agr…

## Selection menu

menu-selection-all = Seleccionar todo ({ $shortcut })
menu-selection-invert = Invertir selección ({ $shortcut })
menu-selection-clear = Quitar selección ({ $shortcut })
menu-selection-delete = Eliminar seleccionados ({ $shortcut })

## View menu

menu-view-navigation-gizmo = Gizmo de navegación
menu-view-search-bar = Barra de búsqueda
menu-view-search-selection-context = Contexto de selección en la búsqueda
menu-view-literal-file-types = Tipos de archivo literales
menu-view-highlight-validator-rows = Resaltar filas del validador
menu-view-context-accumulates = El clic derecho añade a la selección
menu-view-autoscroll-momentum = Inercia del desplazamiento automático
menu-view-motion-effects = Efectos de movimiento
menu-view-selection-pulse = Pulso de selección
menu-view-click-ripples = Ondas al hacer clic
menu-view-icon-micro-motion = Microanimación de iconos
menu-view-animation-demo = Demo de animación (sintética)
menu-view-explorer-association = Abrir .img/.dir desde el Explorador

## Themes menu

theme-dark = Oscuro
theme-light = Claro

## Help menu

menu-help-check-updates = Buscar actualizaciones ({ $shortcut })
menu-help-repository = Visitar el repositorio
menu-help-about = Acerca de

## Language menu

menu-language-system = Idioma del sistema ({ $language })
menu-language-pseudo = Pseudolocalización (prueba de diseño)

## Entry context menu

context-more-selected =
    { $count ->
        [one] +{ $count } seleccionado más
       *[other] +{ $count } seleccionados más
    }
context-play-animation = Reproducir animación
context-open-3d = Abrir en el visor 3D
context-open-external = Abrir en un visor externo
context-view-textures = Ver texturas
context-export-companion-textures = Exportar texturas NFT asociadas
context-export-embedded-textures = Exportar texturas incrustadas
context-export = Exportar
context-rename = Cambiar nombre
context-copy-name = Copiar nombre
context-delete = Eliminar

## Shared dialog buttons and labels

button-close = Cerrar
button-cancel = Cancelar
button-save = Guardar
button-discard = Descartar
button-apply = Aplicar
button-reset = Restablecer
button-convert = Convertir
button-planning = Preparando…
field-format = Formato:
field-name = Nombre:
target-unset = el juego de destino
target-unset-selected = el juego de destino seleccionado

## About dialog

dialog-about-title = Acerca de
about-body =
    IMG Editor Plus v{ $version }

    Un editor de escritorio de archivos IMG de GTA escrito íntegramente en Rust.

    Creado por CloudyTabzy y agentes
    Basado en el IMG Editor original de Grinch_
    (https://github.com/user-grinch/IMGEditor)

    Formatos compatibles:
    - GTA III
    - GTA Vice City
    - GTA San Andreas
    - Bully Scholarship Edition
about-visit-repository = Visitar el repositorio

## Welcome dialog

dialog-welcome-title = Bienvenida
welcome-heading = Bienvenido a { $app } v{ $version }
welcome-tagline = Un editor de archivos IMG de GTA para III, VC, San Andreas y Bully SE.
welcome-dont-show = No volver a mostrar este mensaje
welcome-disable-updates = Desactivar la búsqueda de actualizaciones
welcome-get-started = Empezar

## Unsupported format dialog

dialog-unsupported-title = Formato no compatible
unsupported-body = Este formato IMG no es compatible.
unsupported-path = Ruta: { $path }
unsupported-supported = Formatos compatibles: GTA III, Vice City, San Andreas, Bully SE.

## Texture converter dialogs

format-option-native = { $format } (nativo)
format-option-opt-in = { $format } (opcional)
dxt-high-quality = DXT de alta calidad (cluster fit, más lento)
preview-full-quality = Ver a calidad completa
preview-full-quality-title = { $label } — calidad completa
preview-navigation-hint = Rueda para acercar · arrastra para mover · Esc para cerrar
preview-current = Actual
preview-after = Después (codificada)
dialog-target = Destino: { $target }
dialog-target-archive = Destino: { $target } · { $archive }
dialog-replace-title = Reemplazar textura
replace-summary = Reemplazando '{ $texture }' - origen: { $source } ({ $width }x{ $height })
replace-override-note = Se guarda en memoria como sustitución; el archivo cambia al guardar.
replace-confirm = Reemplazar textura
dialog-new-txd-title = Importar imagen como TXD
new-txd-summary = Nuevo TXD a partir de { $source } ({ $width }x{ $height })
new-txd-name-placeholder = nombre de la textura
new-txd-confirm = Añadir al archivo
dialog-bulk-title = Convertir al dialecto del destino
bulk-entry =
    { $entry } - { $count ->
        [one] { $count } textura
       *[other] { $count } texturas
    } -> { $formats }
bulk-more-entries =
    { $count ->
        [one] … y { $count } entrada más
       *[other] … y { $count } entradas más
    }
bulk-summary =
    Se volverán a codificar para el destino { $textures ->
        [one] { $textures } textura
       *[other] { $textures } texturas
    } de { $entries ->
        [one] { $entries } entrada
       *[other] { $entries } entradas
    } de { $source }.
bulk-skipped = { $skipped } ya nativas (omitidas), { $failed } ilegibles (omitidas).
bulk-ignored =
    { $count ->
        [one] { $count } de las entradas seleccionadas no es un contenedor TXD y no se modificará.
       *[other] { $count } de las entradas seleccionadas no son contenedores TXD y no se modificarán.
    }
bulk-verbatim-note = Las texturas y los nombres no afectados se conservan tal cual; el archivo cambia al guardar.

## Compare with list dialog

dialog-compare-title = Comparar con lista
compare-manifest = Lista: { $path }
compare-archive =
    Archivo: { $archive } · { $count ->
        [one] { $count } entrada
       *[other] { $count } entradas
    }
compare-stats =
    { $names ->
        [one] { $names } nombre en la lista
       *[other] { $names } nombres en la lista
    } ({ $unique } únicos) · { $matched } coincidentes · { $missing } ausentes ({ $missing-unique } únicos)
compare-case-sensitive = Distinguir mayúsculas y minúsculas
compare-show-archive-only = Mostrar entradas que solo están en el archivo
compare-duplicate-lines =
    { $count ->
        [one] { $count } línea duplicada en la lista
       *[other] { $count } líneas duplicadas en la lista
    }
compare-blank-lines =
    { $count ->
        [one] { $count } línea en blanco ignorada
       *[other] { $count } líneas en blanco ignoradas
    }
compare-missing-heading = Ausentes en el archivo ({ $count })
compare-no-missing = No se encontraron entradas ausentes.
compare-more-missing =
    { $count ->
        [one] … y { $count } nombre ausente más
       *[other] … y { $count } nombres ausentes más
    }
compare-archive-only-heading = Solo en el archivo ({ $count })
compare-no-archive-only = No hay entradas que estén solo en el archivo.
compare-more-archive-only =
    { $count ->
        [one] … y { $count } nombre más
       *[other] … y { $count } nombres más
    }
compare-copy-missing = Copiar nombres ausentes
compare-running-heading = Comparando la lista de entradas
compare-running-body = Leyendo { $manifest } y comparándola con { $archive }…

## Save check dialog

dialog-save-check-title = Comprobación antes de guardar
save-check-summary =
    { $textures ->
        [one] { $textures } textura
       *[other] { $textures } texturas
    }, { $entries ->
        [one] { $entries } entrada
       *[other] { $entries } entradas
    } - comprobado para { $target }.
save-check-counts = { $fine } nativas/compatibles · { $convertible } convertibles (sin pérdida) · { $incompatible } incompatibles · { $unknown } desconocidas
save-check-container = Contenedor: { $note }
save-check-anomaly = { $code }: { $count } (p. ej. { $example })
save-check-broken-headers = Cabeceras dañadas (reparables sin volver a codificar):
save-check-warnings =
    { $count ->
        [one] { $count } anomalía de nivel de aviso (se informa, no bloquea).
       *[other] { $count } anomalías de nivel de aviso (se informan, no bloquean).
    }
save-check-repair =
    Reparar cabeceras DXT incoherentes antes de guardar ({ $count ->
        [one] { $count } informe
       *[other] { $count } informes
    }, sin pérdida)
save-check-repair-note = La reparación corrige los campos de cabecera DXT en su sitio; no se vuelve a codificar ningún píxel.
save-check-verbatim-note = Al guardar, cada entrada se escribe tal cual; ninguna textura se vuelve a codificar ni se convierte.
save-check-fix-and-save = Reparar y guardar
save-check-save-anyway = Guardar de todos modos

## Unsaved changes dialog

dialog-unsaved-title = Cambios sin guardar
unsaved-archive = '{ $archive }' tiene cambios sin guardar.
unsaved-archive-note = Si cierras sin guardar, se descartarán; el archivo en disco no se modifica.
unsaved-window =
    { $count ->
        [one] { $count } archivo tiene cambios sin guardar: { $archives }
       *[other] { $count } archivos tienen cambios sin guardar: { $archives }
    }
unsaved-window-note = Si sales ahora, se descartarán; los archivos en disco no se modifican.
unsaved-discard-and-quit = Descartar cambios y salir

## Import check dialog

dialog-import-check-title = Comprobación de importación
import-check-offender = { $name }: { $verdict }
import-check-offender-note = { $name }: { $verdict } - { $note }
import-check-file =
    { $count ->
        [one] { $count } textura
       *[other] { $count } texturas
    }: { $detail }
import-check-more =
    { $count ->
        [one] …y { $count } archivo marcado más.
       *[other] …y { $count } archivos marcados más.
    }
import-check-summary =
    { $flagged } de { $total ->
        [one] { $total } archivo
       *[other] { $total } archivos
    } requieren una decisión para { $target }: { $incompatible ->
        [one] { $incompatible } textura incompatible
       *[other] { $incompatible } texturas incompatibles
    }, { $unknown } desconocidas.
import-check-note = La importación es literal en ambos casos; el formato solo importa si el juego debe cargar estas texturas.
import-check-import-anyway = Importar de todos modos
import-check-cancel = Cancelar importación

## Import folder dialog

dialog-folder-import-title = Importar carpeta
folder-import-path = Carpeta: { $path }
folder-import-summary =
    { $count ->
        [one] { $count } archivo normal
       *[other] { $count } archivos normales
    } • { $size }
folder-import-top-level = Solo se incluyen los archivos que están directamente en esta carpeta; las subcarpetas no se analizan.
folder-import-no-duplicates = No se detectaron nombres duplicados.
folder-import-duplicates =
    { $count ->
        [one] Se detectó { $count } nombre duplicado. Elige cómo tratarlo.
       *[other] Se detectaron { $count } nombres duplicados. Elige cómo tratarlos.
    }
folder-import-skipped =
    { $count ->
        [one] No se pudo inspeccionar { $count } elemento; se omitirá.
       *[other] No se pudieron inspeccionar { $count } elementos; se omitirán.
    }
folder-import-files = Importar archivos
folder-import-skip-duplicates = Importar (omitir duplicados)
folder-import-replace-duplicates = Reemplazar duplicados

## Update check dialog

dialog-update-title = Buscar actualizaciones
update-available = Actualización disponible: { $version }
update-latest = Ya tienes la última versión.
update-failed = No se pudo comprobar si hay actualizaciones: { $error }
update-dont-show = No volver a mostrar este mensaje
update-open-releases = Abrir versiones publicadas

## Validate textures dialog

dialog-validator-title = Validar texturas
validator-intro = Elige el juego al que va destinado este archivo. El validador marca cada textura que queda fuera de los formatos que acepta la versión comercial de ese motor y enumera las desconocidas que podrían cargarse pero no son nativas del juego.
validator-last-run = Última ejecución: { $counts }
validator-no-textures = sin texturas
validator-current-target = destino actual
validator-validate-for = Validar para { $game }
validator-native-heading = Nativos (verificados con la versión comercial):
validator-unknown-heading = Desconocidos / no nativos del juego:
validator-hint-likely = El contenido parece de { $game }
validator-hint-possible = El contenido podría ser de { $game }
validator-hint-current = destino actual: { $game }
validator-use-as-target = Usar { $game } como destino
validator-already-target = (ya es el destino)
validator-highlight-rows = Resaltar filas
legend-native = nativo
legend-native-description = Creado por este motor; no hace falta hacer nada.
legend-supported = compatible / convertible
legend-supported-description = Se carga, pero no es el dialecto de datos del juego; puede ofrecerse una reescritura sin pérdida.
legend-lossy = conversión con pérdida
legend-lossy-description = Solo puede usarse tras una conversión que cambia los píxeles (compresión o cuantización).
legend-unknown = desconocido
legend-unknown-description = No hay pruebas en ningún sentido; no se sabe que sea incompatible. Úsalo con cuidado.
legend-incompatible = incompatible
legend-incompatible-description = El motor seleccionado no puede usar este formato.

## Sort by dialog

dialog-sort-title = Ordenar por — { $archive }
dialog-sort-title-no-archive = Ordenar por — (ningún archivo abierto)
sort-empty = Aún no hay claves. Añade una abajo para empezar a ordenar.
sort-intro = Define reglas de prioridad y aplícalas al archivo actual.
sort-select-key = Elegir clave…
sort-add-key = + Añadir clave
sort-add-key-max = + Añadir clave (máximo alcanzado)
sort-keys-active =
    { $active } de { $total ->
        [one] { $total } clave activa
       *[other] { $total } claves activas
    }
sort-preview-heading = Vista previa en vivo (primeras 10 entradas)
sort-preview-empty = (no hay entradas en el archivo actual)
sort-apply-preset = Aplicar ajuste predefinido…
sort-ascending-short = Asc ▲
sort-descending-short = Desc ▼
sort-key-name = Nombre
sort-key-extension = Extensión
sort-key-type = Tipo
sort-key-size = Tamaño
sort-key-offset = Desplazamiento
sort-key-ide-file = Archivo IDE
sort-key-col-file = Archivo COL
sort-preset-name-az = Nombre (A→Z)
sort-preset-name-za = Nombre (Z→A)
sort-preset-type-then-name = Tipo y luego nombre
sort-preset-size-desc = Tamaño (mayor → menor)
sort-preset-offset-asc = Desplazamiento (menor → mayor)

## Windows file dialogs

file-dialog-open-archive = Abrir archivo IMG
file-dialog-filter-img = Archivo IMG
file-dialog-compare = Comparar con lista de entradas
file-dialog-filter-entry-list = Lista de entradas IMG
file-dialog-open-agr = Abrir grupo de animaciones de Bully
file-dialog-filter-agr = Grupo de animaciones de Bully
file-dialog-import-files = Importar archivos
file-dialog-filter-importable = Archivos importables
file-dialog-import-folder = Selecciona la carpeta que quieres importar
file-dialog-game-folder = Selecciona la carpeta del juego
file-dialog-choose-image = Elige una imagen
file-dialog-filter-images = Imágenes
file-dialog-save-archive = Guardar archivo IMG
file-dialog-export-list = Exportar lista de entradas
file-dialog-export-folder = Selecciona la carpeta de exportación

## Toasts: archives and selection

toast-no-archive-selected = No hay ningún archivo seleccionado.
toast-archive-closed = El archivo ya no está abierto.
toast-target-archive-closed = El archivo de destino ya no está abierto.
toast-target-archive-deselected = El archivo de destino ya no está seleccionado.
toast-validated-archive-closed = Se cerró el archivo que se estaba validando.
toast-entry-unavailable = La entrada seleccionada ya no está disponible.
toast-task-running = Todavía hay otra tarea en curso.
toast-operation-running = Ya hay una operación de archivo en curso.
toast-open-archive-to-validate = Abre primero un archivo para validarlo.
toast-open-archive-to-import = Abre primero un archivo en el que importar.
toast-open-archive-to-import-folder = Abre primero un archivo para importar una carpeta.
toast-open-img-for-model = Abre primero un archivo IMG; el modelo se toma de él.
toast-already-open = Ya está abierto: { $path }
toast-open-failed = No se pudo abrir el archivo: { $error }
toast-file-gone = El archivo ya no existe: { $path }
toast-file-not-found = No se encontró el archivo: { $path }
toast-compare-empty-archive = El archivo seleccionado no tiene entradas que comparar.

## Toasts: saving and packing

toast-archive-saved = Archivo guardado.
toast-save-cancelled = Guardado cancelado.
toast-save-failed = No se pudo guardar: { $error }
toast-save-before-pack = Guarda el archivo antes de compactarlo.
toast-packed = Archivo compactado: se liberaron { $reclaimed } ({ $size } en disco).
toast-packed-nothing = Archivo compactado: no se liberó espacio ({ $size } en disco).
toast-pack-failed = No se pudo compactar: { $error }
toast-headers-repaired =
    { $count ->
        [one] Se reparó { $count } cabecera de textura; guardando.
       *[other] Se repararon { $count } cabeceras de textura; guardando.
    }

## Toasts: game folder

toast-game-folder-set = Carpeta del juego para { $archive }: { $path }
toast-game-folder-automatic = Carpeta del juego para { $archive }: { $path } (automática)
toast-game-folder-none = { $archive } no tiene carpeta del juego.
toast-game-folder-needs-save = Guarda primero el archivo; la carpeta del juego se guarda por archivo.

## Toasts: 3D viewer and animation

toast-select-model = Selecciona primero una entrada NIF, DFF o COL.
toast-viewer-unsupported = El visor 3D integrado admite .nif, .dff y .col ({ $entry }).
toast-select-ifp = Selecciona primero una entrada .ifp.
toast-no-model-for-agr = No se encontró un modelo .nif compatible con { $animation } en este archivo.
toast-no-model-for-agr-open = No se encontró un modelo .nif compatible con { $animation } en el archivo abierto.
toast-loading-animation = Cargando { $animation } en { $model }…
toast-loading-animation-hxd = Cargando { $animation } en { $model }… (catálogo HXD encontrado)
toast-playback-stalled = Reproducción en pausa tras un bloqueo prolongado.
toast-demo-loaded = Demo de animación sintética cargada (sin datos del juego).
toast-demo-closed = Demo de animación cerrada.
toast-animation-ready = Animación lista: { $summary }
toast-animation-failed = No se pudo cargar la animación: { $error }
toast-3d-load-failed = No se pudo cargar en 3D: { $error }
anim-summary-ifp =
    { $animation } en { $model } ({ $clips ->
        [one] { $clips } clip
       *[other] { $clips } clips
    }, GTA IFP)
anim-summary-agr =
    { $animation } en { $model } ({ $clips ->
        [one] { $clips } clip
       *[other] { $clips } clips
    })
anim-summary-agr-named =
    { $animation } en { $model } ({ $clips ->
        [one] { $clips } clip
       *[other] { $clips } clips
    }, nombres del HXD)

## Toasts: textures and conversion

toast-select-texture = Selecciona primero una entrada de textura.
toast-no-target = Este archivo no tiene destino. Elige su juego en Validar texturas.
toast-target-error = { $error } Elige el juego de este archivo en Validar texturas.
toast-replace-busy = Ya se está preparando un reemplazo.
toast-preparing-replacement = Preparando el reemplazo…
toast-replace-failed = No se pudo reemplazar: { $error }
toast-import-busy = Ya se está preparando una importación.
toast-preparing-import = Preparando la importación…
toast-txd-name-required = Ponle un nombre al nuevo TXD.
toast-entry-exists = Ya existe una entrada llamada '{ $name }'.
toast-entry-added = Se añadió '{ $name }'; guarda el archivo para escribirla.
toast-set-target-first = Elige primero un juego de destino (Validar texturas).
toast-select-entries-to-convert = Selecciona primero las entradas que quieres convertir.
toast-archive-changed-planning = El archivo cambió mientras se preparaba la conversión; vuelve a intentarlo.
toast-convert-only-txd = Solo se pueden convertir entradas TXD, y ninguna de las seleccionadas es un contenedor de texturas TXD.
toast-convert-all-native = Todas las texturas seleccionadas ya son nativas para el destino.
toast-archive-changed-after-plan = El archivo cambió después de preparar esta conversión; vuelve a intentarlo.
toast-archive-changed-during = El archivo cambió durante la conversión; se descartaron los resultados obsoletos.
toast-converted =
    { $count ->
        [one] Se convirtió { $count } entrada; guarda el archivo para escribirla.
       *[other] Se convirtieron { $count } entradas; guarda el archivo para escribirlas.
    }
toast-conversion-failed = No se pudo convertir: { $error }
toast-no-decoded-textures = No hay texturas decodificadas para exportar.
toast-decoded =
    { $count ->
        [one] Se decodificó { $count } textura
       *[other] Se decodificaron { $count } texturas
    }
toast-decoded-not-retained =
    { $count ->
        [one] Se decodificó { $count } textura, pero no se pudo conservar la vista previa
       *[other] Se decodificaron { $count } texturas, pero no se pudo conservar la vista previa
    }
toast-pick-embedded-folder = Elige una carpeta para exportar las texturas incrustadas de { $model }
toast-basename-unknown = No se pudo determinar el nombre base de { $entry }
toast-read-failed = No se pudo leer { $name }: { $error }

## Toasts: import and export

toast-import-cancelled = Importación cancelada.
toast-imported =
    { $count ->
        [one] Se importó { $count } archivo.
       *[other] Se importaron { $count } archivos.
    }
toast-imported-unchecked =
    { $count ->
        [one] Se importó { $count } archivo; no hay destino en el validador, así que no se comprobaron los formatos.
       *[other] Se importaron { $count } archivos; no hay destino en el validador, así que no se comprobaron los formatos.
    }
toast-import-failed = No se pudo importar: { $error }
toast-no-files-in-folder = No se encontraron archivos normales en { $folder }.
toast-folder-scan-failed = No se pudo analizar la carpeta: { $error }
toast-folder-import-failed = No se pudo importar la carpeta: { $error }
toast-folder-import-done = Importación de carpeta completada: { $imported } importados, { $skipped } omitidos, { $failed } con error.
toast-folder-import-cancelled = Importación de carpeta cancelada: { $imported } importados, { $skipped } omitidos, { $failed } con error.
toast-see-log = Consulta el registro del archivo para ver los detalles.
toast-exported =
    { $count ->
        [one] Se exportó { $count } entrada.
       *[other] Se exportaron { $count } entradas.
    }
toast-export-failed = No se pudo exportar: { $error }
toast-entry-list-exported =
    { $count ->
        [one] Se exportó { $count } nombre de entrada a { $path }.
       *[other] Se exportaron { $count } nombres de entrada a { $path }.
    }
toast-entry-list-export-failed = No se pudo exportar la lista de entradas: { $error }
toast-compare-failed = No se pudo comparar la lista de entradas: { $error }
toast-no-missing-to-copy = No hay entradas ausentes que copiar.
toast-copied-missing =
    { $count ->
        [one] Se copió { $count } nombre de entrada ausente.
       *[other] Se copiaron { $count } nombres de entrada ausentes.
    }

## Toasts: clipboard, dragging and other actions

toast-copied-entry-details = Se copiaron los detalles de la entrada seleccionada.
toast-copied-logs = Se copió el registro.
toast-copied-name = Nombre copiado: { $name }
toast-drag-cancelled = Arrastre cancelado.
toast-moved-entries =
    { $count ->
        [one] Se movió { $count } entrada al archivo n.º { $archive }.
       *[other] Se movieron { $count } entradas al archivo n.º { $archive }.
    }
toast-autoscroll = Desplazamiento automático activo: mueve el puntero para desplazarte. Haz clic, clic central o clic derecho, usa la rueda o pulsa una tecla para detenerlo.
toast-association-added = IMG Editor Plus ya aparece en «Abrir con» del Explorador para .img/.dir. Para que sea la aplicación predeterminada, elígela en la página de Configuración que se ha abierto.
toast-association-removed = Se quitó la asociación de .img/.dir.
toast-association-failed = No se pudo cambiar la asociación de archivos: { $error }
toast-validation-cancelled = Validación cancelada.
toast-validation-failed = No se pudo validar: { $error }
toast-no-compat-issues = No se encontraron problemas de compatibilidad: { $summary }
validation-summary =
    Se validaron { $txds ->
        [one] { $txds } TXD
       *[other] { $txds } TXD
    } ({ $textures ->
        [one] { $textures } textura
       *[other] { $textures } texturas
    }) para { $game }: { $verdicts }; { $errors ->
        [one] { $errors } error
       *[other] { $errors } errores
    }, { $warnings ->
        [one] { $warnings } aviso
       *[other] { $warnings } avisos
    }

## Archive log

log-viewer-ready = Visor 3D integrado listo
log-viewer-ready-cached = Visor 3D integrado listo (en caché)
log-viewer-opened = Visor 3D abierto: { $name }
log-viewer-failed = Error del visor 3D: { $reason }
log-viewer-closed = Visor 3D cerrado
log-external-viewer = Abriendo el visor 3D externo para { $name }
log-exported =
    { $count ->
        [one] Se exportó { $count } entrada
       *[other] Se exportaron { $count } entradas
    }
log-export-failed = No se pudo exportar: { $error }
log-entry-list-exported =
    { $count ->
        [one] Lista de entradas exportada ({ $count } nombre) a { $path }
       *[other] Lista de entradas exportada ({ $count } nombres) a { $path }
    }
log-compat-check = Comprobación de compatibilidad: { $summary }
log-decoded =
    { $count ->
        [one] Se decodificó { $count } vista previa de textura
       *[other] Se decodificaron { $count } vistas previas de texturas
    }
log-texture-export-failed = No se pudo exportar { $model }: { $error }
recent-exported = Exportado: { $what }
recent-exported-files =
    { $count ->
        [one] { $count } archivo
       *[other] { $count } archivos
    }

## Empty workspace pro tips

pro-tip-label = Consejo:
pro-tip-search = Pulsa Ctrl+F para ir a la búsqueda y usa Arriba/Abajo y Enter para saltar a un resultado.
pro-tip-search-context = Las sugerencias de búsqueda muestran el resultado dentro del archivo; Ver → Contexto de selección en la búsqueda muestra solo los resultados.
pro-tip-context-menu = Haz clic derecho en una entrada para verla en 3D, ver sus texturas, exportarla, cambiarle el nombre y más.
pro-tip-autoscroll = Haz clic central en la lista de entradas para desplazarte como en un navegador; la inercia opcional está en Ver.
pro-tip-tab-keys = Pulsa 1, 2 o 3 para cambiar a Exportar, Vista 3D o Textura.
pro-tip-close-tab = Haz clic central en la pestaña de un archivo para cerrarla rápidamente.
pro-tip-uv-overlay = La superposición UV de las texturas está disponible cuando el modelo seleccionado aporta la geometría correspondiente.
pro-tip-wire-grid = En la vista 3D, la malla muestra las aristas de los triángulos y la cuadrícula del suelo ayuda a juzgar la escala.
pro-tip-save-keys = Usa Ctrl+S para guardar rápido y Ctrl+Shift+S para guardar el archivo con otro nombre.
pro-tip-unique-exports = Las texturas exportadas reciben nombres únicos automáticamente, así que las exportaciones por lotes nunca se sobrescriben.
pro-tip-agr = Editar → Cargar animación .agr reproduce una animación en la vista 3D; Espacio pausa o reanuda y ←/→ avanzan fotograma a fotograma.
pro-tip-model-picker = Mientras se reproduce una animación, el selector de modelo del panel la aplica a cualquier modelo compatible del archivo.
pro-tip-fullscreen-preview = El icono de ampliar en la vista previa de importación o reemplazo la abre a pantalla completa: rueda para acercar, arrastra para mover, Esc para cerrar.
pro-tip-bulk-convert = Convertir la selección al dialecto del destino vuelve a codificar en bloque los TXD seleccionados; elige antes el juego en Validar texturas.
pro-tip-entry-lists = Ctrl+L exporta una lista de entradas y Ctrl+P la compara con el archivo para detectar nombres ausentes.
toast-no-dff-to-animate = No se encontró ningún modelo DFF en este archivo que animar.
toast-replace-needs-txd = El reemplazo funciona con entradas TXD, y esa entrada no lo es.
toast-texture-replaced = Textura reemplazada; guarda el archivo para escribirla.
toast-bully-texture-writing = Aún no se admite escribir texturas de Bully (Gamebryo).
toast-archive-file-missing = El archivo ya no existe en el disco. Usa primero Guardar como…
toast-folder-scan-target-changed = El archivo de destino cambió mientras se analizaba la carpeta.
toast-folder-import-discarded = Se descartó la importación de la carpeta porque el archivo de destino cambió.
toast-compare-discarded = Se descartó la comparación porque el archivo cambió; vuelve a intentarlo.
toast-drop-needs-archive = Abre primero un archivo para soltar en él archivos que no sean IMG.
toast-no-game-root = No se pudo determinar la carpeta del juego a partir de la ruta del archivo.
toast-textures-exported =
    { $count ->
        [one] Se exportó { $count } textura.
       *[other] Se exportaron { $count } texturas.
    }
toast-textures-export-failed = No se pudieron exportar las texturas: { $error }
error-write-file = No se pudo escribir { $path }: { $error }
error-read-entry = No se pudo leer la entrada: { $error }
error-texture-decode = No se pudo decodificar la textura: { $error }
error-texture-preview-unsupported = La vista previa de texturas admite entradas TXD y NFT; '{ $entry }' no es un contenedor de texturas compatible.

## Entry table and search

table-name = Nombre
table-size = Tamaño
table-size-kb = { $size } KB
table-no-matches = Ninguna entrada coincide con el filtro actual.
search-label = Buscar:
search-did-you-mean = Quizás buscabas:
sort-tip-name-asc = Ordenado por nombre de archivo (A → Z).
sort-tip-name-desc = Ordenado por nombre de archivo (Z → A).
sort-tip-name-inactive = Ordenar por nombre de archivo (A → Z).
sort-tip-type-primary = Ordenado por tipo de archivo, primero { $type }.
sort-tip-type-alphabetical = Ordenado por tipo de archivo alfabéticamente.
sort-tip-type-inactive = Ordenar por tipo de archivo (alfabético).
sort-tip-size-desc = Ordenado por tamaño (de mayor a menor).
sort-tip-size-asc = Ordenado por tamaño (de menor a mayor).
sort-tip-size-inactive = Ordenar por tamaño (de mayor a menor).
version-unknown = Desconocido

## Toolbar tooltips

toolbar-new = Nuevo
toolbar-open = Abrir
toolbar-save = Guardar
toolbar-pack = Compactar archivo
toolbar-import = Importar
toolbar-import-folder = Importar carpeta
toolbar-export-selected = Exportar selección
toolbar-delete-selected = Eliminar seleccionados
toolbar-validate = Validar texturas
toolbar-image-as-txd = Importar imagen como TXD
toolbar-convert-selection = Convertir la selección al dialecto del destino

## Empty workspace and status bar

empty-heading = Abre o crea un archivo para empezar.
empty-drop-hint = O arrastra y suelta aquí un archivo .img o .dir para abrirlo.
status-selected = Seleccionadas: { $count }

## Inspector tabs

tab-export = Exportar
tab-3d-view = Vista 3D
tab-texture = Textura

## Export tab

export-format = Formato
export-entries = Entradas
export-entries-value = { $total } (visibles: { $visible })
export-game-folder = Carpeta del juego
export-game-folder-automatic = { $path } (automática)
export-game-folder-none = ninguna
export-game-folder-unsaved = ninguna (archivo sin guardar)
export-progress = Progreso
export-ready = Listo para exportar
export-open-folder = Abrir carpeta de exportación
export-selected-entry = Entrada seleccionada:
export-logs = Registro:
export-recent = Exportaciones recientes:
button-copy = Copiar

## Entry details

inspect-name = Nombre
inspect-type = Tipo
inspect-size = Tamaño
inspect-offset = Desplazamiento
inspect-source = Origen
inspect-size-mb = { $mb } MB ({ $bytes } bytes, { $sectors } sectores)
inspect-size-kb = { $kb } KB ({ $bytes } bytes, { $sectors } sectores)
inspect-size-bytes = { $bytes } bytes ({ $sectors } sectores)
inspect-offset-value = sector { $sector } (byte { $byte })
inspect-hex-preview = Vista previa (hex):

## 3D view tab

viewer-no-archive = No hay ningún archivo abierto.
viewer-try-demo = Probar la demo de animación sintética
viewer-select-model = Selecciona una entrada .nif, .dff o .col para verla en 3D.
viewer-gpu-unavailable = El visor por GPU no está disponible
viewer-gpu-hint = Prueba a borrar la vista previa o a seleccionar un modelo más pequeño.
viewer-clear-error = Borrar el error del visor
viewer-selected-model = el modelo seleccionado
viewer-preparing = Preparando la vista previa 3D
viewer-preparing-detail = Leyendo la geometría y resolviendo las texturas…
viewer-preparing-cache-note = Las próximas vistas previas de este modelo serán instantáneas.
viewer-ready = Listo para ver este modelo en 3D.
viewer-unsupported-entry = El visor integrado muestra entradas .nif, .dff y .col. { $entry } no es un modelo compatible; usa el menú contextual para abrirlo en otro visor.
viewer-load-selected-hint = Usa «{ viewer-load-selected }» arriba para ver este modelo.
viewer-right-click-hint = Selecciona una entrada .nif, .dff o .col y haz clic derecho → { context-open-3d }.
viewer-toolbar-label = 3D:
viewer-preparing-selected = Preparando el modelo seleccionado…
viewer-load-selected = Cargar selección
viewer-load-selected-tip = Carga el modelo seleccionado en el visor 3D.
viewer-reset = Restablecer vista
viewer-reset-tip = Reencuadra la cámara en el modelo. Atajo: R
viewer-clear = Borrar
viewer-clear-tip = Descarta la escena cargada
viewer-wireframe = Malla
viewer-wireframe-tip = Muestra las aristas de los triángulos sobre el modelo sombreado.
viewer-cull = Ocultar caras traseras
viewer-cull-tip = Oculta los triángulos orientados hacia atrás para revisar el orden de las caras.
viewer-textured = Con texturas
viewer-textured-tip = Usa las texturas decodificadas del modelo en lugar de un material neutro.
viewer-alpha = Transparencia
viewer-alpha-tip = Respeta el canal alfa de las texturas para recortes y materiales transparentes.
viewer-alpha-unavailable-tip = Activa «{ viewer-textured }» en un modelo con texturas para usar la transparencia.
viewer-center = Centrar origen
viewer-center-tip = Centra el modelo para inspeccionarlo; desactívalo para conservar las coordenadas del mundo.
viewer-grid = Cuadrícula del suelo
viewer-grid-tip = Muestra la cuadrícula de referencia del mundo y los ejes XYZ.
viewer-stats =
    { $vertices ->
        [one] { $vertices } vértice
       *[other] { $vertices } vértices
    }   { $triangles ->
        [one] { $triangles } triángulo
       *[other] { $triangles } triángulos
    }   { $textures ->
        [one] { $textures } textura
       *[other] { $textures } texturas
    }   { $width }×{ $height }   { $orientation }   { $origin }
viewer-origin-centered = centrado
viewer-origin-world = mundo
viewer-preparing-entry = Preparando { $entry }…
viewer-no-scene = No hay ninguna escena cargada

## Animation dock

anim-preparing = Preparando la animación
anim-preparing-detail = Decodificando clips y resolviendo texturas…
anim-preparing-cache-note = Las próximas reproducciones de esta combinación serán instantáneas.
anim-preparing-label = Preparando { $label }…
anim-title = Animación
anim-title-demo = Demo de animación
anim-demo-note = datos sintéticos, sin datos del juego
anim-exit-demo = Salir de la demo
anim-loop = Repetir reproducción
anim-speed = Velocidad
anim-pack = Paquete de animaciones
anim-model-tip = Reproduce esta animación en otro modelo
anim-clip = Clip
anim-play = Reproducir (Espacio)
anim-pause = Pausa (Espacio)
anim-jump-start = Ir al inicio (Inicio)
anim-step-back = Retroceder un fotograma (←)
anim-step-forward = Avanzar un fotograma (→)
anim-jump-end = Ir al final (Fin)
anim-stop = Detener
anim-rate-source = { $fps } fps de origen
anim-rate-preview = { $fps } fps de vista previa
anim-frame = Encuadre
anim-frame-rest = Reposo
anim-frame-rest-tip = Encuadra la pose de reposo
anim-frame-pose = Pose
anim-frame-pose-tip = Encuadra la pose actual
anim-frame-motion = Movimiento
anim-frame-motion-tip = Encuadra todo el movimiento
anim-in-place-tip = Raíz en el sitio: descarta la traslación de la raíz
anim-follow-tip = La cámara sigue el movimiento de la raíz
anim-skeleton-tip = Muestra el esqueleto superpuesto
anim-motion-path-tip = Muestra la trayectoria de la raíz
anim-ground-tip = Apoya el punto más bajo del clip en el suelo
anim-crossfade-tip = Fundido al cambiar de clip
anim-keys-hint = Espacio reproducir/pausa · ←/→ fotograma · Inicio/Fin extremos · arrastra para desplazarte

## Texture tab

texture-no-archive = No hay ningún archivo abierto.
texture-select-entry = Selecciona una entrada TXD, NFT, NIF o DFF para ver sus texturas.
texture-not-container = { $entry } no es un contenedor de texturas. La vista previa está disponible para entradas TXD, NFT o modelos renderizados.
texture-no-companions = No se resolvieron texturas asociadas para { $entry }.
texture-load-model-hint = Carga el modelo seleccionado para resolver sus texturas.
texture-load-model = Cargar el modelo seleccionado
texture-not-decoded = { $kind } { $entry } aún no está decodificado.
texture-load-textures = Cargar las texturas del { $kind } seleccionado
texture-none-decodable = Este contenedor no tiene texturas decodificables.
texture-animation-model = Modelo de la animación { $entry }; cambia de modelo en el panel 3D
texture-export =
    { $count ->
        [one] Exportar textura ({ $count })
       *[other] Exportar texturas ({ $count })
    }
texture-slot = Textura { $index }/{ $count }
texture-alpha = Alfa:
texture-yes = Sí
texture-no = No
texture-verdict-default = Veredicto para el destino { $game }.
texture-pal8-ready = Apta para PAL8 ({ $colors } colores)
texture-pal8-tip = Cada píxel es uno de estos colores distintos, así que una paleta de 8 bits guarda esta textura sin cuantización.
texture-replace = Reemplazar textura…
texture-replace-hint = Importa PNG/DDS/BMP/TGA y lo vuelve a codificar para el destino del archivo.
texture-uv-standalone-tip = Vista previa de la textura sola. Selecciona un modelo DFF o NIF compatible para activar el mapa UV.
texture-uv-unavailable-tip = El mapa UV está disponible cuando se carga la geometría del modelo correspondiente.
texture-uv-tip = Muestra los triángulos UV de la geometría del modelo correspondiente.
texture-uv = Mostrar mapa UV
texture-uv-standalone = Vista previa de la textura sola · el mapa UV necesita la geometría del modelo
texture-uv-unavailable = Carga la geometría del modelo correspondiente para activar el UV
texture-uv-triangles =
    { $count ->
        [one] { $count } triángulo
       *[other] { $count } triángulos
    }
texture-only-badge = Solo textura
texture-grid = Cuadrícula
texture-grid-tip = Muestra una cuadrícula de referencia sobre la vista previa de la textura.
texture-grid-size-tip = Usa una cuadrícula de referencia de { $size }×{ $size } para la textura.
texture-grid-size = Tamaño:
texture-fullscreen-tip = Ver a pantalla completa (calidad completa)

## Entry table: the Type column and curated file-type names

table-type = Tipo
table-type-sorted = Tipo { $arrow } { $primary }
file-type-model = Modelo
file-type-texture = Textura
file-type-collision = Colisión
file-type-animation = Animación
file-type-placement = Colocación
file-type-definition = Definición
file-type-data = Datos
archive-untitled = Sin título

## Archive log (Export tab)

log-archive-created = Archivo creado
log-archive-saved = Archivo guardado
log-archive-packed =
    { $entries ->
        [one] Archivo compactado: { $entries } entrada, { $reclaimed } recuperados
       *[other] Archivo compactado: { $entries } entradas, { $reclaimed } recuperados
    }
log-imported-entries =
    { $count ->
        [one] Se importó { $count } entrada
       *[other] Se importaron { $count } entradas
    }
log-folder-import = Importación de carpeta: { $imported } importados, { $skipped } omitidos, { $failed } con error
log-folder-import-detail = Detalle de la importación de carpeta: { $detail }
folder-import-cancelled-detail = Importación cancelada; se omitieron los archivos restantes.
folder-import-duplicate = { $name }: duplicado omitido

## Entry details: source and format summary

inspect-source-imported = Importado
inspect-source-imported-from = Importado desde { $path }
inspect-source-archive = Archivo { $archive }, sector { $sector }
inspect-key-format = Formato
inspect-key-version = Versión
inspect-key-clump-size = Tamaño del clump
inspect-key-endian = Orden de bytes
inspect-key-user-version = Versión de usuario
inspect-key-lines = Líneas
inspect-key-atomics = Atomics
inspect-key-textures = Texturas
inspect-key-entries = Entradas
inspect-key-mesh-vertices = Vértices de malla
inspect-key-mesh-faces = Caras de malla
inspect-key-spheres = Esferas
inspect-key-boxes = Cajas
inspect-key-shadow-mesh = Malla de sombra
inspect-rw-truncated = RenderWare (truncado)
inspect-rw-clump = Clump (modelo)
inspect-rw-txd = Diccionario de texturas
inspect-rw-pi-txd = Diccionario de texturas independiente de plataforma
inspect-rw-animation = Animación
inspect-rw-uv-animation = Animación UV
inspect-rw-stream = Flujo RenderWare
inspect-bytes =
    { $count ->
        [one] { $count } byte
       *[other] { $count } bytes
    }
inspect-col-unknown = Colisión desconocida
inspect-endian-big = Big-endian
inspect-endian-little = Little-endian
inspect-nif-truncated = NIF (truncado)
inspect-lines-value = { $lines } ({ $nonempty } no vacías)
inspect-format-scm = Script de GTA (main.scm)
inspect-format-ipl = Colocación de objetos de GTA
inspect-format-ide = Definición de objetos de GTA
inspect-texture-count =
    { $count ->
        [one] { $count } textura
       *[other] { $count } texturas
    }

## Embedded-texture export (NIF → NFT)

texture-export-none = No se encontraron texturas incrustadas
texture-export-done =
    { $count ->
        [one] Se exportó { $count } textura incrustada
       *[other] Se exportaron { $count } texturas incrustadas
    }
texture-export-partial = Texturas exportadas: { $written }; errores: { $failures }
texture-export-no-nft = No se encontró ningún NFT para '{ $name }'

## Manifest comparison errors

compare-manifest-inspect = No se pudo inspeccionar el manifiesto '{ $path }': { $error }
compare-manifest-too-large = El manifiesto '{ $path }' es demasiado grande ({ $size }; el límite es { $limit }).
compare-manifest-read = No se pudo leer el manifiesto '{ $path }': { $error }
compare-manifest-grew = El manifiesto '{ $path }' superó el límite de { $limit } mientras se leía.
compare-manifest-utf8 = El manifiesto '{ $path }' no es UTF-8 válido: { $error }

## Compatibility verdicts

verdict-native = nativo
verdict-supported = compatible
verdict-convertible-lossless = convertible (sin pérdida)
verdict-convertible-lossy = convertible (con pérdida)
verdict-unsupported = incompatible
verdict-untested = sin verificar

## Compatibility evidence notes

compat-class-other-nif = otros formatos NiPixelData
compat-cat-iii-pal = el 96,5 % de las texturas del mundo del juego original
compat-cat-iii-888 = 6806 rásteres, incluido el conjunto de jugador/vehículos
compat-cat-iii-8888 = 1121 rásteres
compat-cat-iii-1555 = 24 rásteres
compat-cat-iii-dxt1 = el juego original no incluye ninguno; el hardware D3D8 lo admite
compat-cat-iii-dxt = valores de compresión D3D8 1-5; el juego original no incluye ninguno
compat-cat-depth24 = forma documentada de profundidad 24; paso/orden sin verificar
compat-cat-iii-565 = asignado por el controlador; III no incluye ninguno
compat-cat-555-lum8 = el controlador asigna C555 y LUM8; el juego original no incluye ninguno
compat-cat-a8l8 = D3D9 y los decodificadores lo admiten; ruta de nibble RW sin verificar
compat-cat-vc-dxt1 = dialecto del mundo original; D3D8 pp=1 (más de 10 000 rásteres, nibbles obsoletos)
compat-cat-vc-dxt3 = dialecto alfa original; D3D8 pp=3 (1149 rásteres)
compat-cat-vc-pal = 27 rásteres; se acepta, pero es poco común
compat-cat-vc-888 = 1 ráster
compat-cat-vc-8888 = III lo incluye; VC no incluye ninguno
compat-cat-vc-565 = las etiquetas 565 del original son datos DXT1; no se midió ningún R565 real
compat-cat-vc-16bit = las etiquetas del original son datos DXT1/DXT3; formas de 16 bits sin medir
compat-cat-vc-dxt = existen valores de compresión D3D8; el original solo usa 1 y 3
compat-cat-sa-dxt1 = 28 807 rásteres en los cuatro archivos
compat-cat-sa-dxt3 = 2098 rásteres
compat-cat-sa-888 = 1015 rásteres, sobre todo skins de player.img
compat-cat-sa-8888 = 237 rásteres
compat-cat-sa-dxt5 = el juego original no incluye ninguno; D3D9 lo admite
compat-cat-sa-dxt24 = la palabra de formato D3D9 los transporta; alfa premultiplicado
compat-cat-sa-pal = las fuentes se contradicen; el original no incluye ninguno; leer y conservar
compat-cat-sa-16bit = asignado por el controlador; el original no incluye 16 bits sin comprimir
compat-cat-sa-a8l8 = Magic.TXD lo incluye para SA PC; ruta RW sin verificar
compat-cat-bully-dxt1 = 31 714 rásteres
compat-cat-bully-dxt5 = 3526 rásteres
compat-cat-bully-rgb = 138 / 134 rásteres
compat-cat-bully-pal = 127 / 1 rásteres
compat-cat-bully-dxt3 = Gamebryo lo admite; el juego original no incluye ninguno
compat-cat-bully-other = 15 rásteres no decodificables

compat-note-bully-not-rw = Los recursos de Bully son NIF/NFT de Gamebryo, no nativos de RenderWare
compat-note-platform-rewrite = un ráster de plataforma { $platform } en un archivo de { $game } necesita reescribir la plataforma/versión
compat-note-no-profile = aún no hay tabla de perfil
compat-note-nft-not-rw = Los rásteres NFT de Gamebryo no son nativos de RenderWare
compat-note-bully-dxt1 = Bully original: 31 714 rásteres DXT1
compat-note-bully-dxt5 = Bully original: 3526 rásteres DXT5
compat-note-bully-rgb = Bully original incluye RGB/RGBA sin comprimir (138/134)
compat-note-bully-pal = Bully original incluye rásteres con paleta (127 PAL + 1 PALA)
compat-note-bully-dxt3 = Gamebryo admite DXT3, pero Bully original no incluye ninguno
compat-note-sa-dxt = SA original: 28 807 rásteres DXT1 + 2098 DXT3
compat-note-sa-dxt24 = el formato nativo D3D9 transporta DXT2/DXT4 (premultiplicado); SA original no incluye ninguno
compat-note-sa-dxt5 = SA original no incluye ninguno; DXT5 funciona gracias a D3D9 (lo usan las herramientas de mods)
compat-note-sa-pal = las fuentes se contradicen sobre las paletas en SA; SA original no incluye ninguna; leer y conservar
compat-note-sa-depth24 = forma documentada R8G8B8 de profundidad 24; SA original no incluye ninguna; ejecución sin verificar
compat-note-sa-16bit = el controlador asigna 1555/565/4444; SA original no incluye 16 bits sin comprimir
compat-note-c555 = el controlador asigna C555 a X1R5G5B5; el original no incluye ninguno
compat-note-lum8 = el controlador asigna LUM8 a D3DFMT_L8; el original no incluye ninguno
compat-note-sa-a8l8 = D3D9 transporta A8L8 y hay decodificadores independientes que lo admiten; la asignación de nibble RW habitual está sin verificar
compat-note-iii-pal = III original: 96,5 % PAL8; VC original: 27 rásteres; se acepta, pero es poco común
compat-note-vc-dxt1 = dialecto del mundo de VC original: DXT1 con D3D8 pp=1; el nibble del ráster es obsoleto
compat-note-iii-dxt1 = III original no incluye rásteres comprimidos (0/15 372); el hardware D3D8 admite DXT1
compat-note-vc-dxt3 = dialecto alfa de VC original: DXT3 con D3D8 pp=3 (1149 rásteres)
compat-note-iii-dxt = los valores de compresión D3D8 1-5 corresponden a DXT1-5 (DXT2/4 premultiplicados); III y VC originales solo usan 1 y 3
compat-note-iii-depth24 = forma documentada R8G8B8 de profundidad 24; paso/orden/ejecución sin verificar
compat-note-iii-8888 = III original incluye 8888 en ambos archivos (1121 rásteres)
compat-note-vc-565 = los rásteres etiquetados 565 de VC original son datos DXT1; no se midió ningún R565 real
compat-note-iii-565 = forma de 16 bits asignada por el controlador; III no incluye ninguna
compat-note-vc-1555 = las etiquetas 1555 de VC original son datos DXT1; no se midió ningún R1555 real
compat-note-vc-4444 = las etiquetas 4444 de VC original son datos DXT3; no se midió ningún R4444 real
compat-note-iii-4444 = III no incluye ninguno; 16 bits con alfa de la época D3D8
compat-note-iii-a8l8 = Magic.TXD incluye A8L8 para SA PC; D3D9 lo transporta; ruta RW habitual sin verificar

## Output-format choices (import and replace dialogs)

compat-choice-iii-888 = X8R8G8B8 de 32 bits, sin pérdida; estándar de txd.img en III original
compat-choice-iii-8888 = A8R8G8B8, conserva el alfa; III original incluye 1121
compat-choice-iii-pal8 = paleta de 8 bits, cuantiza los colores; dialecto del mundo original (96,5 %)
compat-choice-pal4 = paleta de 4 bits, cuantiza mucho; solo para arte de 16 colores
compat-choice-iii-1555 = 16 bits con alfa de 1 bit; el original incluye 24
compat-choice-iii-dxt = admitido por el hardware, pero III no lo usa; con pérdida
compat-choice-vc-dxt1 = dialecto del mundo de VC original (D3D8 pp=1); compresión con pérdida
compat-choice-vc-dxt3 = dialecto alfa de VC original (D3D8 pp=3); compresión con pérdida
compat-choice-vc-888 = X8R8G8B8 de 32 bits, sin pérdida; VC original incluye uno
compat-choice-vc-8888 = A8R8G8B8, conserva el alfa; forma de la época D3D8
compat-choice-vc-pal8 = paleta de 8 bits, cuantiza los colores; VC original incluye 27
compat-choice-vc-565 = 16 bits; VC original etiqueta 565 a datos DXT, forma sin comprimir sin medir
compat-choice-vc-4444 = 16 bits con alfa; VC original etiqueta 4444 a datos DXT3
compat-choice-player-888 = X8R8G8B8 de 32 bits; player.img original incluye 269
compat-choice-player-8888 = A8R8G8B8, conserva el alfa; player.img original incluye 125
compat-choice-player-dxt = compatible en todas partes; con pérdida (player.img no incluye ninguno)
compat-choice-sa-dxt1 = dialecto del mundo de SA original; compresión con pérdida
compat-choice-sa-dxt3 = dialecto alfa de SA original; compresión con pérdida
compat-choice-sa-8888 = A8R8G8B8, sin pérdida; SA original lo incluye en player.img
compat-choice-sa-pal8 = cuantiza los colores; SA original no incluye rásteres con paleta

## Conversion warnings and errors

compat-warn-alpha-discarded = { $source } tiene alfa, pero { $format } no puede guardarlo: se descartará el canal alfa.
compat-warn-dxt-lossy = La compresión { $format } es con pérdida; la vista previa muestra el resultado codificado.
compat-warn-palette-exact =
    { $colors ->
        [one] { $format } guarda la imagen exactamente ({ $colors } color).
       *[other] { $format } guarda la imagen exactamente ({ $colors } colores).
    }
compat-warn-palette-quantize = Los colores se cuantizarán a { $cap } como máximo.
compat-warn-stream-budget = { $width }x{ $height } supera el límite de streaming de 1024 px de SA; puede que el juego no lo cargue.
compat-warn-verdict = { $format } es { $verdict } para { $game }: { $note }
compat-error-image-format = formato de imagen no reconocido: { $error }
compat-error-image-kind = las imágenes { $kind } no son compatibles; usa PNG, DDS, BMP o TGA
compat-error-decode = no se pudo decodificar { $label }: { $error }
compat-error-image-size = tamaño de imagen no compatible: { $width }x{ $height }
compat-error-unreadable-texture = '{ $name }' no se puede decodificar ({ $error }); la conversión necesita píxeles legibles
compat-error-no-dimensions = la textura no tiene dimensiones
compat-error-texture-index = el índice de textura { $index } está fuera de rango
compat-error-unknown-target = juego de destino desconocido: '{ $id }'

## Save check: container conventions

compat-container-v2-expects-v1 = Contenedor IMG v2, pero { $game } espera IMG v1: el juego no verá estos archivos.
compat-container-v1-sa = Contenedor IMG v1; San Andreas original usa IMG v2 (v1 solo se carga si aparece en gta.dat).
compat-container-xbox = Empaquetado de Xbox 360; { $game } espera un contenedor de PC.

## Import check notes

compat-scan-unreadable = no se pudo leer: { $error }
compat-scan-bad-nif = cabecera NIF ilegible: { $error }
compat-scan-empty-nif = sin bloques NiPixelData (archivo vacío)
compat-scan-not-texture = no es una textura TXD ni de Gamebryo

## Target-game suggestion evidence

compat-hint-gamebryo = { $total } entradas de Gamebryo ({ $nft } NFT, { $nif } NIF)
compat-hint-d3d9 = plataforma 9 (D3D9) en el { $share } de { $total } rásteres analizados
compat-hint-pal = PAL8/PAL4 en el { $share } de los rásteres analizados
compat-hint-vc16 = 16 bits 565/4444/1555 en el { $share } de los rásteres analizados
compat-hint-d3d8 = plataforma 8 (D3D8)
