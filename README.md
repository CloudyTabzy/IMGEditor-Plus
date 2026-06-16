# IMGEditor v2.0 — Rust port

A pure Rust desktop editor for GTA IMG archives, built with [Iced](https://iced.rs/) and Tokio.

## Supported formats

- IMG v1: GTA III, GTA Vice City, Bully Scholarship Edition
- IMG v2: GTA San Andreas

## Building

Requires Rust **1.96+** and a Windows desktop environment.

```powershell
cargo build --release
```

The binary is produced at `target\release\imgeditor.exe`.

## Packaging a release

Run the packaging script from the project root:

```powershell
.\package-release.ps1
```

This builds a release binary and copies it into `dist\` along with the file-association notes.

## Windows file association

See [docs/windows_file_association.md](docs/windows_file_association.md).

## Features

- Open, save, import, and export IMG v1 and v2 archives.
- Native file dialogs (can be disabled with `--no-default-features`).
- Drag-and-drop file opening into the active archive tab.
- Searchable entry list with aligned Name / Type / Size columns.
- Theme selection (System / Light / Catppuccin Mocha / Tokyo Night / Gruvbox Dark).
- Lucide icon toolbar for common actions.
- Persistent window size, position, last-used folders, and theme.
- Background GitHub update checks.
- Keyboard shortcuts matching the original editor.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl + N | New archive |
| Ctrl + O | Open archive |
| Ctrl + S | Save archive in place |
| Shift + S | Save archive as |
| Ctrl + I | Import files |
| Shift + I | Import and replace |
| Ctrl + E | Export all |
| Shift + E | Export selected |
| Ctrl + A | Select all |
| Shift + A | Invert selection |
| Shift + X | Close selected archive |
| Delete | Delete selected entries |
