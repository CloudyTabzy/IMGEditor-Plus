//! Explorer integration for `.img` / `.dir` archives, per user (HKCU).
//!
//! Since Windows 8 only the user can pick a file type's default app, so this
//! does not try to take over `.img` (whose default is usually Windows' own
//! disk-image "Mount"). It registers a ProgID with the open command and icon,
//! lists it under each extension's `OpenWithProgids` (Explorer's "Open with"),
//! and declares the app through `RegisteredApplications`, which gives it a
//! page in Settings > Default apps where the user can make it the default.

use std::path::Path;

pub const PROG_ID: &str = "IMGEditorPlus.Archive";
pub const EXTENSIONS: [&str; 2] = [".img", ".dir"];
pub const APP_NAME: &str = "IMG Editor Plus";

const APP_KEY: &str = r"Software\IMGEditorPlus";
const CAPABILITIES_KEY: &str = r"Software\IMGEditorPlus\Capabilities";
const PROG_ID_KEY: &str = r"Software\Classes\IMGEditorPlus.Archive";
const REGISTERED_APPLICATIONS_KEY: &str = r"Software\RegisteredApplications";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationState {
    /// Registered and pointing at the running executable.
    Registered,
    /// Registered for another executable path (moved or a different build).
    RegisteredElsewhere,
    NotRegistered,
    /// Not a Windows build.
    Unsupported,
}

/// The shell command Explorer runs to open a file with this executable.
pub fn open_command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

/// Every registry value the association writes, relative to
/// `HKEY_CURRENT_USER`: `(key, value name, data)`, where an empty name is the
/// key's default value.
pub(crate) fn association_values(exe: &Path) -> Vec<(String, String, String)> {
    let value = |key: &str, name: &str, data: String| (key.to_string(), name.to_string(), data);
    let mut values = vec![
        value(PROG_ID_KEY, "", "GTA IMG archive".to_string()),
        value(
            &format!(r"{PROG_ID_KEY}\DefaultIcon"),
            "",
            format!("\"{}\",0", exe.display()),
        ),
        value(
            &format!(r"{PROG_ID_KEY}\shell\open\command"),
            "",
            open_command(exe),
        ),
        value(
            &format!(r"{PROG_ID_KEY}\Application"),
            "ApplicationName",
            APP_NAME.to_string(),
        ),
        value(CAPABILITIES_KEY, "ApplicationName", APP_NAME.to_string()),
        value(
            CAPABILITIES_KEY,
            "ApplicationDescription",
            "Open and edit GTA III, Vice City, San Andreas and Bully IMG archives.".to_string(),
        ),
        value(
            REGISTERED_APPLICATIONS_KEY,
            APP_NAME,
            CAPABILITIES_KEY.to_string(),
        ),
    ];
    for extension in EXTENSIONS {
        values.push(value(
            &format!(r"Software\Classes\{extension}\OpenWithProgids"),
            PROG_ID,
            String::new(),
        ));
        values.push(value(
            &format!(r"{CAPABILITIES_KEY}\FileAssociations"),
            extension,
            PROG_ID.to_string(),
        ));
    }
    values
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::path::Path;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

    use super::*;

    fn current_user() -> RegKey {
        RegKey::predef(HKEY_CURRENT_USER)
    }

    /// A missing key or value means there is nothing left to remove.
    fn ignore_missing(result: io::Result<()>) -> io::Result<()> {
        match result {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    pub(crate) fn state_in(root: &RegKey, exe: &Path) -> AssociationState {
        let command = root
            .open_subkey(format!(r"{PROG_ID_KEY}\shell\open\command"))
            .and_then(|key| key.get_value::<String, _>(""));
        match command {
            Ok(command) if command.eq_ignore_ascii_case(&open_command(exe)) => {
                AssociationState::Registered
            }
            Ok(_) => AssociationState::RegisteredElsewhere,
            Err(_) => AssociationState::NotRegistered,
        }
    }

    pub(crate) fn register_in(root: &RegKey, exe: &Path) -> io::Result<()> {
        for (key, name, data) in association_values(exe) {
            let (key, _) = root.create_subkey(&key)?;
            key.set_value(&name, &data)?;
        }
        Ok(())
    }

    /// Removes only what [`register_in`] wrote; other apps' entries under
    /// the shared extension keys are left untouched.
    pub(crate) fn unregister_in(root: &RegKey) -> io::Result<()> {
        ignore_missing(root.delete_subkey_all(PROG_ID_KEY))?;
        ignore_missing(root.delete_subkey_all(APP_KEY))?;
        for extension in EXTENSIONS {
            if let Ok(key) = root.open_subkey_with_flags(
                format!(r"Software\Classes\{extension}\OpenWithProgids"),
                KEY_SET_VALUE,
            ) {
                ignore_missing(key.delete_value(PROG_ID))?;
            }
        }
        if let Ok(key) = root.open_subkey_with_flags(REGISTERED_APPLICATIONS_KEY, KEY_SET_VALUE) {
            ignore_missing(key.delete_value(APP_NAME))?;
        }
        Ok(())
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(
            event_id: i32,
            flags: u32,
            item1: *const std::ffi::c_void,
            item2: *const std::ffi::c_void,
        );
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_cmd: i32,
        ) -> *mut std::ffi::c_void;
    }

    /// Tell Explorer that associations changed so icons and "Open with"
    /// refresh without a sign-out.
    fn notify_shell() {
        const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
        const SHCNF_IDLIST: u32 = 0;
        // SAFETY: SHCNE_ASSOCCHANGED takes no items; the documented call
        // passes null for both, and the function only reads its arguments.
        unsafe {
            SHChangeNotify(
                SHCNE_ASSOCCHANGED,
                SHCNF_IDLIST,
                std::ptr::null(),
                std::ptr::null(),
            );
        }
    }

    pub fn state() -> AssociationState {
        match std::env::current_exe() {
            Ok(exe) => state_in(&current_user(), &exe),
            Err(_) => AssociationState::NotRegistered,
        }
    }

    pub fn register() -> io::Result<()> {
        let exe = std::env::current_exe()?;
        register_in(&current_user(), &exe)?;
        notify_shell();
        Ok(())
    }

    pub fn unregister() -> io::Result<()> {
        unregister_in(&current_user())?;
        notify_shell();
        Ok(())
    }

    /// Open this app's page in Settings > Default apps, where the user
    /// confirms it as the default for `.img` / `.dir`. The URI goes through
    /// the shell's protocol handler; `explorer <uri>` rejects the query string
    /// and opens a Documents window instead.
    pub fn open_default_apps_settings() {
        let uri = format!(
            "ms-settings:defaultapps?registeredAppUser={}",
            APP_NAME.replace(' ', "%20")
        );
        let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() };
        let operation = wide("open");
        let file = wide(&uri);
        const SW_SHOWNORMAL: i32 = 1;
        // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive
        // the call; null window, parameters and directory are documented as
        // valid. The returned pseudo-HINSTANCE is only an error code.
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::io;

    use super::AssociationState;

    pub fn state() -> AssociationState {
        AssociationState::Unsupported
    }

    pub fn register() -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "file associations are Windows-only",
        ))
    }

    pub fn unregister() -> io::Result<()> {
        Ok(())
    }

    pub fn open_default_apps_settings() {}
}

pub use platform::{open_default_apps_settings, register, state, unregister};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_command_quotes_the_exe_and_the_file_argument() {
        let exe = Path::new(r"C:\Program Files\IMG Editor\imgeditor.exe");
        assert_eq!(
            open_command(exe),
            r#""C:\Program Files\IMG Editor\imgeditor.exe" "%1""#
        );
    }

    #[test]
    fn association_lists_both_extensions_without_touching_their_defaults() {
        let values = association_values(Path::new(r"C:\apps\imgeditor.exe"));
        for extension in EXTENSIONS {
            let open_with = format!(r"Software\Classes\{extension}\OpenWithProgids");
            assert!(
                values
                    .iter()
                    .any(|(key, name, _)| key == &open_with && name == PROG_ID)
            );
            // The extension key's own default value (its current default
            // handler) is never written.
            assert!(!values.iter().any(|(key, name, _)| {
                key == &format!(r"Software\Classes\{extension}") && name.is_empty()
            }));
        }
    }

    /// Round-trips the real registry code inside a throwaway HKCU key, so
    /// the user's actual associations are never touched.
    #[cfg(windows)]
    #[test]
    fn register_and_unregister_round_trip_in_a_scratch_key() {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;

        let scratch_path = format!(
            r"Software\IMGEditorPlusTests\association-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (root, _) = hkcu.create_subkey(&scratch_path).unwrap();
        struct Cleanup<'a>(&'a RegKey, &'a str);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete_subkey_all(self.1);
                let _ = self.0.delete_subkey(r"Software\IMGEditorPlusTests");
            }
        }
        let _cleanup = Cleanup(&hkcu, &scratch_path);

        // Another app's entry under the shared extension key must survive.
        let (other, _) = root
            .create_subkey(r"Software\Classes\.img\OpenWithProgids")
            .unwrap();
        other.set_value("Windows.IsoFile", &String::new()).unwrap();

        let exe = Path::new(r"C:\apps\imgeditor.exe");
        assert_eq!(
            platform::state_in(&root, exe),
            AssociationState::NotRegistered
        );
        platform::register_in(&root, exe).unwrap();
        assert_eq!(platform::state_in(&root, exe), AssociationState::Registered);
        assert_eq!(
            platform::state_in(&root, Path::new(r"D:\moved\imgeditor.exe")),
            AssociationState::RegisteredElsewhere
        );

        platform::unregister_in(&root).unwrap();
        assert_eq!(
            platform::state_in(&root, exe),
            AssociationState::NotRegistered
        );
        let open_with = root
            .open_subkey(r"Software\Classes\.img\OpenWithProgids")
            .unwrap();
        assert!(open_with.get_value::<String, _>(PROG_ID).is_err());
        assert!(open_with.get_value::<String, _>("Windows.IsoFile").is_ok());
        // Unregistering twice is harmless.
        platform::unregister_in(&root).unwrap();
    }
}
