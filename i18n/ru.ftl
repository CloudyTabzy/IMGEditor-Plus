### IMG Editor Plus - Русский.
###
### Draft translation; pending review by a native speaker.
### Keep message ids and { $variables } exactly as in en.ftl.
### Russian plurals use the CLDR categories one / few / many / other.

## Menu bar: root menus

menu-file = Файл
menu-recent = Недавние
menu-edit = Правка
menu-selection = Выделение
menu-view = Вид
menu-themes = Темы
menu-help = Справка
menu-language = Язык

## File menu

menu-file-new = Создать ({ $shortcut })
menu-file-open = Открыть… ({ $shortcut })
menu-file-save = Сохранить ({ $shortcut })
menu-file-save-as = Сохранить как… ({ $shortcut })
menu-file-pack = Упаковать архив
menu-file-set-game-folder = Выбрать папку игры…
menu-file-reset-game-folder = Сбросить папку игры
menu-file-close-tab = Закрыть вкладку ({ $shortcut })
menu-file-sort-by = Сортировка…

## Recent menu

menu-recent-empty = Нет недавних файлов

## Edit menu

menu-edit-import = Импорт ({ $shortcut })
menu-edit-import-folder = Импорт папки
menu-edit-export-all = Экспортировать всё ({ $shortcut })
menu-edit-export-selected = Экспортировать выбранное ({ $shortcut })
menu-edit-export-list = Экспортировать список ({ $shortcut })
menu-edit-compare-list = Сравнить со списком ({ $shortcut })
menu-edit-load-agr = Загрузить анимацию .agr…

## Selection menu

menu-selection-all = Выделить всё ({ $shortcut })
menu-selection-invert = Инвертировать выделение ({ $shortcut })
menu-selection-clear = Снять выделение ({ $shortcut })
menu-selection-delete = Удалить выбранное ({ $shortcut })

## View menu

menu-view-navigation-gizmo = Навигационный гизмо
menu-view-search-bar = Строка поиска
menu-view-search-selection-context = Контекст выделения в поиске
menu-view-literal-file-types = Буквальные типы файлов
menu-view-highlight-validator-rows = Подсвечивать строки валидатора
menu-view-context-accumulates = Правый клик добавляет к выделению
menu-view-autoscroll-momentum = Инерция автопрокрутки
menu-view-motion-effects = Эффекты движения
menu-view-selection-pulse = Пульсация выделения
menu-view-click-ripples = Волны при нажатии
menu-view-icon-micro-motion = Микроанимация значков
menu-view-animation-demo = Демо анимации (синтетическое)
menu-view-explorer-association = Открывать .img/.dir из Проводника

## Themes menu

theme-dark = Тёмная
theme-light = Светлая

## Help menu

menu-help-check-updates = Проверить обновления ({ $shortcut })
menu-help-repository = Открыть репозиторий
menu-help-about = О программе

## Language menu

menu-language-system = Язык системы ({ $language })
menu-language-pseudo = Псевдолокаль (проверка вёрстки)

## Entry context menu

context-more-selected =
    { $count ->
        [one] ещё { $count } выбранный элемент
        [few] ещё { $count } выбранных элемента
        [many] ещё { $count } выбранных элементов
       *[other] ещё { $count } выбранного элемента
    }
context-play-animation = Воспроизвести анимацию
context-open-3d = Открыть в 3D-просмотрщике
context-open-external = Открыть во внешнем просмотрщике
context-view-textures = Просмотреть текстуры
context-export-companion-textures = Экспортировать связанные текстуры NFT
context-export-embedded-textures = Экспортировать встроенные текстуры
context-export = Экспорт
context-rename = Переименовать
context-copy-name = Копировать имя
context-delete = Удалить
