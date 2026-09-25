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

## Toasts: archives and selection

toast-no-archive-selected = Архив не выбран.
toast-archive-closed = Архив уже закрыт.
toast-target-archive-closed = Целевой архив уже закрыт.
toast-target-archive-deselected = Целевой архив больше не выбран.
toast-validated-archive-closed = Проверяемый архив был закрыт.
toast-entry-unavailable = Выбранная запись больше недоступна.
toast-task-running = Другая задача ещё выполняется.
toast-operation-running = Операция с архивом уже выполняется.
toast-open-archive-to-validate = Сначала откройте архив, чтобы проверить его.
toast-open-archive-to-import = Сначала откройте архив, в который нужно импортировать.
toast-open-archive-to-import-folder = Сначала откройте архив, чтобы импортировать папку.
toast-open-img-for-model = Сначала откройте архив IMG — модель берётся из него.
toast-already-open = Уже открыт: { $path }
toast-open-failed = Не удалось открыть архив: { $error }
toast-file-gone = Файл больше не существует: { $path }
toast-file-not-found = Файл не найден: { $path }
toast-compare-empty-archive = В выбранном архиве нет записей для сравнения.

## Toasts: saving and packing

toast-archive-saved = Архив сохранён.
toast-save-cancelled = Сохранение отменено.
toast-save-failed = Не удалось сохранить: { $error }
toast-save-before-pack = Сохраните архив, прежде чем упаковывать его.
toast-packed = Архив упакован — освобождено { $reclaimed } (на диске { $size }).
toast-packed-nothing = Архив упакован — место не освобождено (на диске { $size }).
toast-pack-failed = Не удалось упаковать: { $error }
toast-headers-repaired =
    { $count ->
        [one] Исправлен { $count } заголовок текстуры; сохранение.
        [few] Исправлено { $count } заголовка текстур; сохранение.
        [many] Исправлено { $count } заголовков текстур; сохранение.
       *[other] Исправлено { $count } заголовка текстуры; сохранение.
    }

## Toasts: game folder

toast-game-folder-set = Папка игры для { $archive }: { $path }
toast-game-folder-automatic = Папка игры для { $archive }: { $path } (автоматически)
toast-game-folder-none = У { $archive } нет папки игры.
toast-game-folder-needs-save = Сначала сохраните архив: папка игры хранится для каждого файла архива.

## Toasts: 3D viewer and animation

toast-select-model = Сначала выберите запись NIF, DFF или COL.
toast-viewer-unsupported = Встроенный 3D-просмотрщик поддерживает .nif, .dff и .col ({ $entry }).
toast-select-ifp = Сначала выберите запись .ifp.
toast-no-model-for-agr = Для { $animation } в этом архиве не найдена подходящая модель .nif.
toast-no-model-for-agr-open = Для { $animation } в открытом архиве не найдена подходящая модель .nif.
toast-loading-animation = Загрузка { $animation } на { $model }…
toast-loading-animation-hxd = Загрузка { $animation } на { $model }… (найден каталог HXD)
toast-playback-stalled = Воспроизведение приостановлено после долгой задержки.
toast-demo-loaded = Загружено синтетическое демо анимации (без данных игры).
toast-demo-closed = Демо анимации закрыто.
toast-animation-ready = Анимация готова: { $summary }
toast-animation-failed = Не удалось загрузить анимацию: { $error }
toast-3d-load-failed = Не удалось загрузить в 3D: { $error }
anim-summary-ifp = { $animation } на { $model } (клипов: { $clips }, GTA IFP)
anim-summary-agr = { $animation } на { $model } (клипов: { $clips })
anim-summary-agr-named = { $animation } на { $model } (клипов: { $clips }, имена из HXD)

## Toasts: textures and conversion

toast-select-texture = Сначала выберите запись с текстурами.
toast-no-target = Для этого архива не выбрана цель. Выберите его игру в окне «Проверка текстур».
toast-target-error = { $error } Выберите игру этого архива в окне «Проверка текстур».
toast-replace-busy = Замена уже готовится.
toast-preparing-replacement = Подготовка замены…
toast-replace-failed = Не удалось заменить: { $error }
toast-import-busy = Импорт уже готовится.
toast-preparing-import = Подготовка импорта…
toast-txd-name-required = Укажите имя нового TXD.
toast-entry-exists = Запись «{ $name }» уже существует.
toast-entry-added = Добавлено «{ $name }» — сохраните архив, чтобы записать изменения.
toast-set-target-first = Сначала выберите целевую игру («Проверка текстур»).
toast-select-entries-to-convert = Сначала выберите записи для преобразования.
toast-archive-changed-planning = Архив изменился во время подготовки преобразования; повторите попытку.
toast-convert-only-txd = Преобразовать можно только записи TXD, а среди выбранных нет контейнеров текстур TXD.
toast-convert-all-native = Все выбранные текстуры уже в родном формате целевой игры.
toast-archive-changed-after-plan = Архив изменился после подготовки преобразования; повторите попытку.
toast-archive-changed-during = Архив изменился во время преобразования; устаревшие результаты отброшены.
toast-converted =
    { $count ->
        [one] Преобразована { $count } запись — сохраните архив, чтобы записать изменения.
        [few] Преобразованы { $count } записи — сохраните архив, чтобы записать изменения.
        [many] Преобразовано { $count } записей — сохраните архив, чтобы записать изменения.
       *[other] Преобразовано { $count } записи — сохраните архив, чтобы записать изменения.
    }
toast-conversion-failed = Не удалось преобразовать: { $error }
toast-no-decoded-textures = Нет декодированных текстур для экспорта.
toast-decoded = Декодировано текстур: { $count }
toast-decoded-not-retained = Декодировано текстур: { $count }, но сохранить предпросмотр не удалось
toast-pick-embedded-folder = Выберите папку для экспорта встроенных текстур из { $model }
toast-basename-unknown = Не удалось определить базовое имя { $entry }
toast-read-failed = Не удалось прочитать { $name }: { $error }

## Toasts: import and export

toast-import-cancelled = Импорт отменён.
toast-imported = Импортировано файлов: { $count }.
toast-imported-unchecked = Импортировано файлов: { $count }. Целевая игра не выбрана, поэтому форматы не проверялись.
toast-import-failed = Не удалось импортировать: { $error }
toast-no-files-in-folder = В { $folder } нет обычных файлов.
toast-folder-scan-failed = Не удалось просканировать папку: { $error }
toast-folder-import-failed = Не удалось импортировать папку: { $error }
toast-folder-import-done = Импорт папки завершён. Импортировано: { $imported }, пропущено: { $skipped }, с ошибками: { $failed }.
toast-folder-import-cancelled = Импорт папки отменён. Импортировано: { $imported }, пропущено: { $skipped }, с ошибками: { $failed }.
toast-see-log = Подробности — в журнале архива.
toast-exported = Экспортировано записей: { $count }.
toast-export-failed = Не удалось экспортировать: { $error }
toast-entry-list-exported = Экспортировано имён записей: { $count } — в { $path }.
toast-entry-list-export-failed = Не удалось экспортировать список записей: { $error }
toast-compare-failed = Не удалось сравнить список записей: { $error }
toast-no-missing-to-copy = Нет отсутствующих записей для копирования.
toast-copied-missing = Скопировано отсутствующих имён: { $count }.

## Toasts: clipboard, dragging and other actions

toast-copied-entry-details = Сведения о выбранной записи скопированы.
toast-copied-logs = Журнал скопирован.
toast-copied-name = Имя скопировано: { $name }
toast-drag-cancelled = Перетаскивание отменено.
toast-moved-entries = Перемещено записей: { $count } — в архив № { $archive }.
toast-autoscroll = Автопрокрутка включена: двигайте указатель для прокрутки. Чтобы остановить, щёлкните любой кнопкой мыши, покрутите колесо или нажмите клавишу.
toast-association-added = IMG Editor Plus добавлен в меню «Открыть с помощью» Проводника для .img/.dir. Чтобы сделать его программой по умолчанию, выберите его на открывшейся странице «Параметров».
toast-association-removed = Сопоставление .img/.dir удалено.
toast-association-failed = Не удалось изменить сопоставление файлов: { $error }
toast-validation-cancelled = Проверка отменена.
toast-validation-failed = Не удалось выполнить проверку: { $error }
toast-no-compat-issues = Проблем совместимости не найдено — { $summary }
validation-summary = Проверено для { $game } — TXD: { $txds }, текстур: { $textures }: { $verdicts }; ошибок: { $errors }, предупреждений: { $warnings }

## Archive log

log-viewer-ready = Встроенный 3D-просмотрщик готов
log-viewer-ready-cached = Встроенный 3D-просмотрщик готов (из кэша)
log-viewer-opened = 3D-просмотрщик открыт: { $name }
log-viewer-failed = Ошибка 3D-просмотрщика: { $reason }
log-viewer-closed = 3D-просмотрщик закрыт
log-external-viewer = Открытие внешнего 3D-просмотрщика для { $name }
log-exported = Экспортировано записей: { $count }
log-export-failed = Не удалось экспортировать: { $error }
log-entry-list-exported = Список записей экспортирован (имён: { $count }) в { $path }
log-compat-check = Проверка совместимости: { $summary }
log-decoded = Декодировано предпросмотров текстур: { $count }
log-texture-export-failed = Не удалось экспортировать { $model }: { $error }
recent-exported = Экспортировано: { $what }
recent-exported-files =
    { $count ->
        [one] { $count } файл
        [few] { $count } файла
        [many] { $count } файлов
       *[other] { $count } файла
    }

## Empty workspace pro tips

pro-tip-label = Совет:
pro-tip-search = Нажмите Ctrl+F, чтобы перейти к поиску, затем используйте стрелки вверх/вниз и Enter для перехода к совпадению.
pro-tip-search-context = Подсказки поиска показывают совпадение в контексте архива; Вид → Контекст выделения в поиске показывает только результаты.
pro-tip-context-menu = Щёлкните запись правой кнопкой мыши, чтобы открыть её в 3D, посмотреть текстуры, экспортировать, переименовать и не только.
pro-tip-autoscroll = Щёлкните список записей средней кнопкой мыши для автопрокрутки как в браузере; инерция включается в меню «Вид».
pro-tip-tab-keys = Нажмите 1, 2 или 3, чтобы перейти на вкладку «Экспорт», «3D-вид» или «Текстура».
pro-tip-close-tab = Щёлкните вкладку архива средней кнопкой мыши, чтобы быстро закрыть её.
pro-tip-uv-overlay = Наложение UV на текстуру доступно, когда выбранная модель содержит подходящую геометрию.
pro-tip-wire-grid = В 3D-виде каркас показывает рёбра треугольников, а сетка пола помогает оценить масштаб.
pro-tip-save-keys = Ctrl+S — быстрое сохранение, Ctrl+Shift+S — сохранить архив под новым именем.
pro-tip-unique-exports = Экспортируемые текстуры автоматически получают уникальные имена, поэтому пакетный экспорт ничего не перезаписывает.
pro-tip-agr = Правка → Загрузить анимацию .agr воспроизводит анимацию в 3D-виде; пробел ставит на паузу и продолжает, а ←/→ листают кадры.
pro-tip-model-picker = Во время воспроизведения выбор модели на панели проигрывает анимацию на любой совместимой модели из архива.
pro-tip-fullscreen-preview = Значок развёртывания на предпросмотре импорта или замены открывает его на весь экран: колесо — масштаб, перетаскивание — сдвиг, Esc — закрыть.
pro-tip-bulk-convert = «Преобразовать выделение для целевой игры» перекодирует выбранные TXD разом — сначала выберите игру в окне «Проверка текстур».
pro-tip-entry-lists = Ctrl+L экспортирует список записей, а Ctrl+P сравнивает его с архивом, чтобы найти отсутствующие имена.
toast-no-dff-to-animate = В этом архиве нет модели DFF для анимации.
toast-replace-needs-txd = Замена работает только с записями TXD, а эта запись — не TXD.
toast-texture-replaced = Текстура заменена — сохраните архив, чтобы записать изменения.
toast-bully-texture-writing = Запись текстур Bully (Gamebryo) пока не поддерживается.
toast-archive-file-missing = Файла архива больше нет на диске. Сначала воспользуйтесь «Сохранить как…».
toast-folder-scan-target-changed = Целевой архив изменился во время сканирования папки.
toast-folder-import-discarded = Импорт папки отменён: целевой архив изменился.
toast-compare-discarded = Сравнение отменено: архив изменился; повторите попытку.
toast-drop-needs-archive = Сначала откройте архив, чтобы перетаскивать в него файлы, отличные от IMG.
toast-no-game-root = Не удалось определить папку игры по пути к архиву.
toast-textures-exported = Экспортировано текстур: { $count }.
toast-textures-export-failed = Не удалось экспортировать текстуры: { $error }
error-write-file = Не удалось записать { $path }: { $error }
error-read-entry = Не удалось прочитать запись: { $error }
error-texture-decode = Не удалось декодировать текстуру: { $error }
error-texture-preview-unsupported = Предпросмотр текстур поддерживает записи TXD и NFT; «{ $entry }» — не поддерживаемый контейнер текстур.

## Entry table and search

table-name = Имя
table-size = Размер
table-size-kb = { $size } КБ
table-no-matches = Нет записей, подходящих под текущий фильтр.
search-label = Поиск:
search-did-you-mean = Возможно, вы имели в виду:
sort-tip-name-asc = Отсортировано по имени файла (А → Я).
sort-tip-name-desc = Отсортировано по имени файла (Я → А).
sort-tip-name-inactive = Сортировать по имени файла (А → Я).
sort-tip-type-primary = Отсортировано по типу файла, сначала { $type }.
sort-tip-type-alphabetical = Отсортировано по типу файла по алфавиту.
sort-tip-type-inactive = Сортировать по типу файла (по алфавиту).
sort-tip-size-desc = Отсортировано по размеру (сначала большие).
sort-tip-size-asc = Отсортировано по размеру (сначала маленькие).
sort-tip-size-inactive = Сортировать по размеру (сначала большие).
version-unknown = Неизвестно

## Toolbar tooltips

toolbar-new = Создать
toolbar-open = Открыть
toolbar-save = Сохранить
toolbar-pack = Упаковать архив
toolbar-import = Импорт
toolbar-import-folder = Импорт папки
toolbar-export-selected = Экспортировать выбранное
toolbar-delete-selected = Удалить выбранное
toolbar-validate = Проверить текстуры
toolbar-image-as-txd = Импорт изображения как TXD
toolbar-convert-selection = Преобразовать выделение для целевой игры

## Empty workspace and status bar

empty-heading = Откройте или создайте архив, чтобы начать.
empty-drop-hint = Или перетащите сюда файл .img или .dir, чтобы открыть его.
status-selected = Выбрано: { $count }

## Inspector tabs

tab-export = Экспорт
tab-3d-view = 3D-вид
tab-texture = Текстура

## Export tab

export-format = Формат
export-entries = Записей
export-entries-value = { $total } (видно: { $visible })
export-game-folder = Папка игры
export-game-folder-automatic = { $path } (автоматически)
export-game-folder-none = нет
export-game-folder-unsaved = нет (архив не сохранён)
export-progress = Ход выполнения
export-ready = Готово к экспорту
export-open-folder = Открыть папку экспорта
export-selected-entry = Выбранная запись:
export-logs = Журнал:
export-recent = Недавний экспорт:
button-copy = Копировать

## Entry details

inspect-name = Имя
inspect-type = Тип
inspect-size = Размер
inspect-offset = Смещение
inspect-source = Источник
inspect-size-mb = { $mb } МБ ({ $bytes } байт, секторов: { $sectors })
inspect-size-kb = { $kb } КБ ({ $bytes } байт, секторов: { $sectors })
inspect-size-bytes = { $bytes } байт (секторов: { $sectors })
inspect-offset-value = сектор { $sector } (байт { $byte })
inspect-hex-preview = Предпросмотр (hex):

## 3D view tab

viewer-no-archive = Архив не открыт.
viewer-try-demo = Попробовать синтетическое демо анимации
viewer-select-model = Выберите запись .nif, .dff или .col, чтобы посмотреть её в 3D.
viewer-gpu-unavailable = GPU-просмотрщик недоступен
viewer-gpu-hint = Попробуйте очистить предпросмотр или выбрать модель поменьше.
viewer-clear-error = Сбросить ошибку просмотрщика
viewer-selected-model = выбранная модель
viewer-preparing = Подготовка 3D-предпросмотра
viewer-preparing-detail = Чтение геометрии и поиск текстур…
viewer-preparing-cache-note = В следующий раз эта модель откроется мгновенно.
viewer-ready = Модель готова к просмотру в 3D.
viewer-unsupported-entry = Встроенный просмотрщик показывает записи .nif, .dff и .col. { $entry } — неподдерживаемая модель; откройте её в другом просмотрщике через контекстное меню.
viewer-load-selected-hint = Нажмите «{ viewer-load-selected }» выше, чтобы посмотреть эту модель.
viewer-right-click-hint = Выберите запись .nif, .dff или .col, затем правый клик → { context-open-3d }.
viewer-toolbar-label = 3D:
viewer-preparing-selected = Подготовка выбранной модели…
viewer-load-selected = Загрузить выбранное
viewer-load-selected-tip = Загрузить выбранную модель в 3D-просмотрщик.
viewer-reset = Сбросить вид
viewer-reset-tip = Заново навести камеру на модель. Клавиша: R
viewer-clear = Очистить
viewer-clear-tip = Выгрузить загруженную сцену
viewer-wireframe = Каркас
viewer-wireframe-tip = Показывать рёбра треугольников поверх затенённой модели.
viewer-cull = Скрывать обратные грани
viewer-cull-tip = Скрывать треугольники, повёрнутые назад, чтобы проверить порядок обхода граней.
viewer-textured = Текстуры
viewer-textured-tip = Использовать декодированные текстуры модели вместо нейтрального материала.
viewer-alpha = Прозрачность
viewer-alpha-tip = Учитывать альфа-канал текстур для вырезов и прозрачных материалов.
viewer-alpha-unavailable-tip = Включите «{ viewer-textured }» на модели с текстурами, чтобы использовать прозрачность.
viewer-center = Центрировать
viewer-center-tip = Переместить модель в центр для осмотра; отключите, чтобы сохранить мировые координаты.
viewer-grid = Сетка пола
viewer-grid-tip = Показывать опорную сетку мира и оси XYZ.
viewer-stats = Вершин: { $vertices }   треугольников: { $triangles }   текстур: { $textures }   { $width }×{ $height }   { $orientation }   { $origin }
viewer-origin-centered = по центру
viewer-origin-world = мировые координаты
viewer-preparing-entry = Подготовка { $entry }…
viewer-no-scene = Сцена не загружена

## Animation dock

anim-preparing = Подготовка анимации
anim-preparing-detail = Декодирование клипов и поиск текстур…
anim-preparing-cache-note = В следующий раз эта пара откроется мгновенно.
anim-preparing-label = Подготовка { $label }…
anim-title = Анимация
anim-title-demo = Демо анимации
anim-demo-note = синтетические данные — без данных игры
anim-exit-demo = Выйти из демо
anim-loop = Повторять воспроизведение
anim-speed = Скорость
anim-pack = Набор анимаций
anim-model-tip = Проиграть эту анимацию на другой модели
anim-clip = Клип
anim-play = Воспроизвести (пробел)
anim-pause = Пауза (пробел)
anim-jump-start = В начало (Home)
anim-step-back = На кадр назад (←)
anim-step-forward = На кадр вперёд (→)
anim-jump-end = В конец (End)
anim-stop = Остановить
anim-rate-source = { $fps } кадр/с (исходная)
anim-rate-preview = { $fps } кадр/с (предпросмотр)
anim-frame = Кадрировать
anim-frame-rest = Покой
anim-frame-rest-tip = Навести камеру на позу покоя
anim-frame-pose = Поза
anim-frame-pose-tip = Навести камеру на текущую позу
anim-frame-motion = Движение
anim-frame-motion-tip = Навести камеру на всё движение
anim-in-place-tip = Корень на месте — отбросить перемещение корня
anim-follow-tip = Камера следует за движением корня
anim-skeleton-tip = Показывать скелет
anim-motion-path-tip = Показывать траекторию корня
anim-ground-tip = Ставить нижнюю точку клипа на пол
anim-crossfade-tip = Плавный переход при смене клипа
anim-keys-hint = Пробел — пуск/пауза · ←/→ — кадр · Home/End — края · перетаскивание — перемотка

## Texture tab

texture-no-archive = Архив не открыт.
texture-select-entry = Выберите запись TXD, NFT, NIF или DFF, чтобы посмотреть текстуры.
texture-not-container = { $entry } — не контейнер текстур. Предпросмотр доступен для записей TXD, NFT и отрисованных моделей.
texture-no-companions = Для { $entry } не найдено связанных текстур.
texture-load-model-hint = Загрузите выбранную модель, чтобы найти её текстуры.
texture-load-model = Загрузить выбранную модель
texture-not-decoded = { $kind } { $entry } ещё не декодирован.
texture-load-textures = Загрузить текстуры выбранного { $kind }
texture-none-decodable = В этом контейнере нет декодируемых текстур.
texture-animation-model = Модель анимации { $entry } — сменить модель можно на панели 3D
texture-export = Экспортировать текстуры ({ $count })
texture-slot = Текстура { $index }/{ $count }
texture-alpha = Альфа:
texture-yes = Да
texture-no = Нет
texture-verdict-default = Оценка для цели { $game }.
texture-pal8-ready = Подходит для PAL8 (цветов: { $colors })
texture-pal8-tip = Каждый пиксель — один из этих цветов, поэтому 8-битная палитра сохранит текстуру без квантования.
texture-replace = Заменить текстуру…
texture-replace-hint = Импортирует PNG/DDS/BMP/TGA и перекодирует для целевой игры архива.
texture-uv-standalone-tip = Отдельный предпросмотр текстуры. Выберите подходящую модель DFF или NIF, чтобы включить UV-развёртку.
texture-uv-unavailable-tip = UV-развёртка станет доступна после загрузки геометрии подходящей модели.
texture-uv-tip = Показывать UV-треугольники из геометрии подходящей модели.
texture-uv = Показывать UV-развёртку
texture-uv-standalone = Только текстура · для UV нужна геометрия подходящей модели
texture-uv-unavailable = Загрузите геометрию подходящей модели, чтобы включить UV
texture-uv-triangles = Треугольников: { $count }
texture-only-badge = Только текстура
texture-grid = Сетка
texture-grid-tip = Показывать опорную сетку поверх предпросмотра текстуры.
texture-grid-size-tip = Использовать опорную сетку { $size }×{ $size } для текстуры.
texture-grid-size = Размер:
texture-fullscreen-tip = На весь экран (полное качество)
