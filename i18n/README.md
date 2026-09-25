# Translations

IMG Editor Plus is translated with [Project Fluent](https://projectfluent.org/)
using its Rust implementation (`fluent-bundle`, `fluent-syntax`). Each language
is one `.ftl` file here, embedded in the executable at build time.

| File | Language | Status |
|---|---|---|
| `en.ftl` | English | Source of truth |
| `es.ftl` | Español | Draft, pending native review |
| `ru.ftl` | Русский | Draft, pending native review |

The Language menu switches instantly, without a restart. "System language"
follows the Windows display language and falls back to English.

## Translating

Copy a message's id from `en.ftl` and give it a translated value:

```ftl
menu-file-save = Guardar ({ $shortcut })
```

- Keep ids and `{ $variables }` exactly as in English. You may drop a
  variable your language does not need, but you cannot add one; the build
  fails if you do.
- A message you leave out is shown in English.
- Brand, format and file names stay untranslated: IMG Editor Plus, TXD, DFF,
  NIF, `.img`. Keyboard shortcuts arrive already formatted ("Ctrl + N").
- For counts, use Fluent's plural selectors. Russian uses the categories
  `one` (1, 21, 31…), `few` (2–4, 22–24…) and `many` (5–20, 25–30…):

```ftl
context-more-selected =
    { $count ->
        [one] ещё { $count } выбранный элемент
        [few] ещё { $count } выбранных элемента
        [many] ещё { $count } выбранных элементов
       *[other] ещё { $count } выбранного элемента
    }
```

Build with `cargo build`. It reports syntax errors with file and line, rejects
messages English does not have, and prints a warning listing how many
messages are still untranslated.

## Adding user-facing text (developers)

User-facing text never goes into Rust source as a literal.

1. Add the message to `en.ftl`, next to related ones. Ids are lowercase
   kebab-case and grouped by where they appear (`menu-…`, `context-…`).
2. Call the generated function: `build/i18n.rs` turns every `en.ftl` message
   into a typed function in `crate::i18n::t`, so `menu-file-new` with
   `{ $shortcut }` becomes `t::menu_file_new(shortcut)`. A misspelled
   message or a missing, extra or misnamed argument is a compile error.
3. Add the translations to `es.ftl` and `ru.ftl`, or leave them for a
   translator (they fall back to English).

Pass counts as numbers, not formatted strings, so plural rules can apply.

## Checking layouts

Spanish and Russian run roughly a third longer than English. Debug builds
add a "Pseudo-locale" entry to the Language menu: English with every letter
accented and the vowels doubled ("Fíílée"), so clipped or badly wrapped
labels show up before real translations exist.

Headings use the Bricolage display font, which has no Cyrillic; in Russian
they switch to Inter ExtraBold (`fonts::display_font`).
