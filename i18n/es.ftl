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
