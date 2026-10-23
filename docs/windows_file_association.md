# Windows file association

IMG Editor Plus can register itself as an "Open with" handler for `.img` and
`.dir` archives without taking over the file type.

## In-app setup

1. Open **View → Open .img/.dir from Explorer**.
2. The editor registers a per-user ProgID (`IMGEditorPlus.Archive`) under
   `HKCU`, lists it under both extensions' `OpenWithProgids`, and declares
   itself through `RegisteredApplications`.
3. It then opens **Settings → Default apps** at the IMG Editor Plus page.
   Pick IMG Editor Plus for `.img` and/or `.dir` there. Windows 8 and later
   only let the user choose a default app, so the editor never writes the
   extension's default handler itself.
4. Double-clicking an associated archive opens it in the editor. The same
   menu item toggles the registration off.

No administrator rights are required: everything is written under the current
user's registry hive.

## Notes

- This editor supports PC IMG v1 (GTA III / Vice City / Bully), the
  big-endian Bully Xbox 360 IMG v1 variant, and IMG v2 (GTA San Andreas).
  Windows does not distinguish these formats by extension, so the app
  validates the directory/header structure and detects the variant
  automatically before opening it.
- `.img` is also used by Windows' own disk-image "Mount" handler, which is why
  the editor adds an additional handler instead of replacing the default.
- An archive path passed on the command line (for example from Explorer's
  "Open") opens on startup.
