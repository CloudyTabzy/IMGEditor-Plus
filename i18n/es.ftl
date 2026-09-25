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
