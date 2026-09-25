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

## Shared dialog buttons and labels

button-close = Закрыть
button-cancel = Отмена
button-save = Сохранить
button-discard = Не сохранять
button-apply = Применить
button-reset = Сбросить
button-convert = Преобразовать
button-planning = Подготовка…
field-format = Формат:
field-name = Имя:
# Genitive: used after "для" and "из".
target-unset = целевой игры
target-unset-selected = выбранной целевой игры

## About dialog

dialog-about-title = О программе
about-body =
    IMG Editor Plus v{ $version }

    Настольный редактор архивов GTA IMG, полностью написанный на Rust.

    Авторы: CloudyTabzy и агенты
    Основан на оригинальном IMG Editor от Grinch_
    (https://github.com/user-grinch/IMGEditor)

    Поддерживаемые форматы:
    - GTA III
    - GTA Vice City
    - GTA San Andreas
    - Bully Scholarship Edition
about-visit-repository = Открыть репозиторий

## Welcome dialog

dialog-welcome-title = Добро пожаловать
welcome-heading = Добро пожаловать в { $app } v{ $version }
welcome-tagline = Редактор архивов GTA для III, VC, San Andreas и Bully SE.
welcome-dont-show = Больше не показывать это сообщение
welcome-disable-updates = Отключить проверку обновлений
welcome-get-started = Начать

## Unsupported format dialog

dialog-unsupported-title = Неподдерживаемый формат
unsupported-body = Этот формат IMG не поддерживается.
unsupported-path = Путь: { $path }
unsupported-supported = Поддерживаемые форматы: GTA III, Vice City, San Andreas, Bully SE.

## Texture converter dialogs

format-option-native = { $format } (родной)
format-option-opt-in = { $format } (по выбору)
dxt-high-quality = DXT высокого качества (cluster fit, медленнее)
preview-full-quality = Открыть в полном качестве
preview-full-quality-title = { $label } — полное качество
preview-navigation-hint = Колесо — масштаб · перетаскивание — сдвиг · Esc — закрыть
preview-current = Сейчас
preview-after = После (закодировано)
dialog-target = Цель: { $target }
dialog-target-archive = Цель: { $target } · { $archive }
dialog-replace-title = Замена текстуры
replace-summary = Замена «{ $texture }» — источник: { $source } ({ $width }x{ $height })
replace-override-note = Хранится в памяти как замена; файл архива изменится при сохранении.
replace-confirm = Заменить текстуру
dialog-new-txd-title = Импорт изображения как TXD
new-txd-summary = Новый TXD из { $source } ({ $width }x{ $height })
new-txd-name-placeholder = имя текстуры
new-txd-confirm = Добавить в архив
dialog-bulk-title = Преобразование для целевой игры
bulk-entry =
    { $entry } — { $count ->
        [one] { $count } текстура
        [few] { $count } текстуры
        [many] { $count } текстур
       *[other] { $count } текстуры
    } -> { $formats }
bulk-more-entries =
    { $count ->
        [one] … и ещё { $count } запись
        [few] … и ещё { $count } записи
        [many] … и ещё { $count } записей
       *[other] … и ещё { $count } записи
    }
bulk-summary = Будет перекодировано для целевой игры — текстур: { $textures }, записей: { $entries } (из { $source }).
bulk-skipped = Уже родные (пропущены): { $skipped }; не читаются (пропущены): { $failed }.
bulk-ignored = Выбранных записей не в формате TXD (они не изменятся): { $count }.
bulk-verbatim-note = Нетронутые текстуры и имена сохраняются как есть; архив изменится при сохранении.

## Compare with list dialog

dialog-compare-title = Сравнение со списком
compare-manifest = Список: { $path }
compare-archive = Архив: { $archive } · записей: { $count }
compare-stats = Имён в списке: { $names } (уникальных: { $unique }) · совпало: { $matched } · отсутствует: { $missing } (уникальных: { $missing-unique })
compare-case-sensitive = Учитывать регистр
compare-show-archive-only = Показывать записи, которых нет в списке
compare-duplicate-lines = Повторяющихся строк в списке: { $count }
compare-blank-lines = Пропущено пустых строк: { $count }
compare-missing-heading = Нет в архиве ({ $count })
compare-no-missing = Отсутствующих записей не найдено.
compare-more-missing = … и ещё { $count }
compare-archive-only-heading = Только в архиве ({ $count })
compare-no-archive-only = Записей, которые есть только в архиве, не найдено.
compare-more-archive-only = … и ещё { $count }
compare-copy-missing = Копировать отсутствующие имена
compare-running-heading = Сравнение списка записей
compare-running-body = Чтение { $manifest } и сверка с { $archive }…

## Save check dialog

dialog-save-check-title = Проверка перед сохранением
save-check-summary = Текстур: { $textures }, записей: { $entries } — проверено для { $target }.
save-check-counts = Родные/поддерживаемые: { $fine } · преобразуемые (без потерь): { $convertible } · несовместимые: { $incompatible } · неизвестные: { $unknown }
save-check-container = Контейнер: { $note }
save-check-anomaly = { $code }: { $count } (напр. { $example })
save-check-broken-headers = Повреждённые заголовки (исправляются без перекодирования):
save-check-warnings = Аномалий уровня предупреждения: { $count } (показаны, не мешают сохранению).
save-check-repair = Исправить несогласованные заголовки DXT перед сохранением (отчётов: { $count }, без потерь)
save-check-repair-note = Исправление правит поля заголовка DXT на месте; ни один пиксель не перекодируется.
save-check-verbatim-note = При сохранении каждая запись пишется как есть; текстуры не перекодируются и не преобразуются.
save-check-fix-and-save = Исправить и сохранить
save-check-save-anyway = Всё равно сохранить

## Unsaved changes dialog

dialog-unsaved-title = Несохранённые изменения
unsaved-archive = В «{ $archive }» есть несохранённые изменения.
unsaved-archive-note = Если закрыть без сохранения, они будут потеряны; файл архива на диске не изменится.
unsaved-window = Архивов с несохранёнными изменениями: { $count } — { $archives }
unsaved-window-note = Если выйти сейчас, они будут потеряны; файлы на диске не изменятся.
unsaved-discard-and-quit = Выйти без сохранения

## Import check dialog

dialog-import-check-title = Проверка импорта
import-check-offender = { $name }: { $verdict }
import-check-offender-note = { $name }: { $verdict } — { $note }
import-check-file = Текстур: { $count } — { $detail }
import-check-more = …и ещё файлов с пометками: { $count }.
import-check-summary =
    Требуют решения для { $target }: { $flagged } из { $total ->
        [one] { $total } файла
       *[other] { $total } файлов
    }; несовместимых текстур: { $incompatible }, неизвестных: { $unknown }.
import-check-note = Импорт в любом случае выполняется без изменений; формат важен, только если игра должна загружать эти текстуры.
import-check-import-anyway = Всё равно импортировать
import-check-cancel = Отменить импорт

## Import folder dialog

dialog-folder-import-title = Импорт папки
folder-import-path = Папка: { $path }
folder-import-summary = Обычных файлов: { $count } • { $size }
folder-import-top-level = Учитываются только файлы непосредственно в этой папке; вложенные папки не сканируются.
folder-import-no-duplicates = Повторяющихся имён не обнаружено.
folder-import-duplicates = Обнаружено повторяющихся имён: { $count }. Выберите, что с ними делать.
folder-import-skipped = Не удалось проверить элементов: { $count }; они будут пропущены.
folder-import-files = Импортировать файлы
folder-import-skip-duplicates = Импорт (пропустить повторы)
folder-import-replace-duplicates = Заменить повторы

## Update check dialog

dialog-update-title = Проверка обновлений
update-available = Доступно обновление: { $version }
update-latest = У вас последняя версия.
update-failed = Не удалось проверить обновления: { $error }
update-dont-show = Больше не показывать это сообщение
update-open-releases = Открыть страницу релизов

## Validate textures dialog

dialog-validator-title = Проверка текстур
validator-intro = Выберите игру, для которой предназначен этот архив. Проверка отмечает каждую текстуру вне форматов, которые принимает розничная версия движка, и перечисляет неизвестные форматы, которые могут загрузиться, но не являются родными для игры.
validator-last-run = Последняя проверка: { $counts }
validator-no-textures = нет текстур
validator-current-target = текущая цель
validator-validate-for = Проверить для { $game }
validator-native-heading = Родные (проверено на розничной версии):
validator-unknown-heading = Неизвестные / не родные для игры:
validator-hint-likely = Содержимое похоже на { $game }
validator-hint-possible = Содержимое, возможно, для { $game }
validator-hint-current = текущая цель: { $game }
validator-use-as-target = Выбрать { $game } целью
validator-already-target = (уже выбрана целью)
validator-highlight-rows = Подсвечивать строки
legend-native = родной
legend-native-description = Создано этим движком — ничего делать не нужно.
legend-supported = поддерживается / преобразуется
legend-supported-description = Загружается, но это не родной формат данных игры; может быть предложена перезапись без потерь.
legend-lossy = преобразование с потерями
legend-lossy-description = Можно использовать только после преобразования с изменением пикселей (сжатие или квантование).
legend-unknown = неизвестно
legend-unknown-description = Нет данных ни в ту, ни в другую сторону — несовместимость не доказана. Используйте осторожно.
legend-incompatible = несовместимо
legend-incompatible-description = Выбранный движок не может использовать этот формат.

## Sort by dialog

dialog-sort-title = Сортировка — { $archive }
dialog-sort-title-no-archive = Сортировка — (архив не открыт)
sort-empty = Ключей пока нет. Добавьте ключ ниже, чтобы начать сортировку.
sort-intro = Задайте правила приоритета и примените их к текущему архиву.
sort-select-key = Выберите ключ…
sort-add-key = + Добавить ключ
sort-add-key-max = + Добавить ключ (достигнут максимум)
sort-keys-active = Активно ключей: { $active } из { $total }
sort-preview-heading = Предпросмотр (первые 10 записей)
sort-preview-empty = (в текущем архиве нет записей)
sort-apply-preset = Применить шаблон…
sort-ascending-short = По возр. ▲
sort-descending-short = По убыв. ▼
sort-key-name = Имя
sort-key-extension = Расширение
sort-key-type = Тип
sort-key-size = Размер
sort-key-offset = Смещение
sort-key-ide-file = Файл IDE
sort-key-col-file = Файл COL
sort-preset-name-az = Имя (А→Я)
sort-preset-name-za = Имя (Я→А)
sort-preset-type-then-name = Тип, затем имя
sort-preset-size-desc = Размер (больше → меньше)
sort-preset-offset-asc = Смещение (меньше → больше)

## Windows file dialogs

file-dialog-open-archive = Открыть архив IMG
file-dialog-filter-img = Архив IMG
file-dialog-compare = Сравнить со списком записей
file-dialog-filter-entry-list = Список записей IMG
file-dialog-open-agr = Открыть группу анимаций Bully
file-dialog-filter-agr = Группа анимаций Bully
file-dialog-import-files = Импорт файлов
file-dialog-filter-importable = Импортируемые файлы
file-dialog-import-folder = Выберите папку для импорта
file-dialog-game-folder = Выберите папку игры
file-dialog-choose-image = Выберите изображение
file-dialog-filter-images = Изображения
file-dialog-save-archive = Сохранить архив IMG
file-dialog-export-list = Экспорт списка записей
file-dialog-export-folder = Выберите папку для экспорта
