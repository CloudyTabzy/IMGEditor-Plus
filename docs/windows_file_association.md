# Windows file association

You can make Windows open `.img` archives directly in IMGEditor by associating the `.img` extension with the release binary.

## Manual setup

1. Build or download the release binary: `imgeditor.exe`.
2. Place the binary in a permanent location (e.g. `C:\Tools\IMGEditor\imgeditor.exe`).
3. Open an elevated Command Prompt or PowerShell and run:

```cmd
reg add "HKCR\.img" /ve /d "IMGEditor.img" /f
reg add "HKCR\IMGEditor.img\shell\open\command" /ve /d "\"C:\Tools\IMGEditor\imgeditor.exe\" \"%1\"" /f
```

Replace `C:\Tools\IMGEditor\imgeditor.exe` with the actual path to your binary.

4. Restart File Explorer or log out and back in. Double-clicking an `.img` file should now open it in IMGEditor.

## Notes

- This editor supports IMG v1 (GTA III / Vice City / Bully) and IMG v2 (GTA San Andreas). Windows does not distinguish between these formats by extension, so the app detects the format automatically when opening.
- File association is not created automatically by the editor; it must be set up by the user or an installer.
