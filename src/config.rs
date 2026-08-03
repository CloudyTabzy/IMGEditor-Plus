use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Maximum number of recent files retained in the MRU list. Bounded
/// so the settings file stays a reasonable size and the menu doesn't
/// scroll off the screen.
pub const RECENT_FILES_MAX: usize = 10;

/// One entry in the recent-files list. `path` is canonicalized at
/// touch time so the same file opened via different mount points or
/// slashes doesn't appear twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFile {
    pub path: PathBuf,
}

impl RecentFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Filename without the directory, suitable for a menu label.
    pub fn display_name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    }

    /// Directory containing the file, for the menu's secondary line.
    pub fn display_dir(&self) -> &str {
        self.path
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
    }
}

/// Most-recently-used file list. Touching a path that's already present
/// moves it to the front; touching a new path evicts the oldest entry
/// past `RECENT_FILES_MAX`. Order is MRU-first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentFiles {
    entries: Vec<RecentFile>,
}

impl RecentFiles {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `path` (or move to front if already present) and evict
    /// overflow. Canonicalizes the path so the same file via
    /// `C:/foo` and `C:\foo` collapses to one entry. Errors during
    /// canonicalization fall back to the input path — a best-effort
    /// dedupe, not a guarantee.
    pub fn touch<P: AsRef<Path>>(&mut self, path: P) {
        let canonical = path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| path.as_ref().to_path_buf());
        self.entries.retain(|e| e.path != canonical);
        self.entries.insert(0, RecentFile::new(canonical));
        if self.entries.len() > RECENT_FILES_MAX {
            self.entries.truncate(RECENT_FILES_MAX);
        }
    }

    /// Explicitly remove a path (e.g. user clicked "Remove from list").
    pub fn remove<P: AsRef<Path>>(&mut self, path: P) {
        self.entries.retain(|e| e.path != path.as_ref());
    }

    /// Wipe the list.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Borrowed, MRU-first iteration. Yields `(index, &RecentFile)`.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &RecentFile)> {
        self.entries.iter().enumerate()
    }

    /// MRU-first iteration over only the entries that still exist
    /// on disk. Used by the "Open Recent" menu to skip broken links
    /// without mutating the stored list.
    pub fn iter_existing(&self) -> impl Iterator<Item = (usize, &RecentFile)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.path.exists())
    }

    /// Number of stored entries (not filtered for existence).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build a compact display label for the menu, trimming the
    /// directory when it would push the line over `max_chars`.
    pub fn menu_label(&self, index: usize, max_chars: usize) -> String {
        let Some((_, entry)) = self.entries.iter().enumerate().nth(index) else {
            return String::new();
        };
        let name = entry.display_name();
        let dir = entry.display_dir();
        let prefix = format!("{}. ", index + 1);
        let available = max_chars.saturating_sub(prefix.len() + name.len() + 3);
        if dir.is_empty() || available == 0 {
            format!("{prefix}{name}")
        } else {
            let trimmed = if dir.len() > available {
                let take = available.saturating_sub(1);
                if take == 0 {
                    String::new()
                } else {
                    format!("…{}", &dir[dir.len() - take..])
                }
            } else {
                dir.to_string()
            };
            if trimmed.is_empty() {
                format!("{prefix}{name}")
            } else {
                format!("{prefix}{trimmed} / {name}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    DarkCatppuccin,
    DarkTokyoNight,
    DarkGruvbox,
    DarkEverforest,
}

impl ThemeMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ThemeMode::System => "Dark",
            ThemeMode::Light => "Light",
            ThemeMode::DarkCatppuccin => "Catppuccin Mocha",
            ThemeMode::DarkTokyoNight => "Tokyo Night",
            ThemeMode::DarkGruvbox => "Gruvbox",
            ThemeMode::DarkEverforest => "Everforest",
        }
    }

    pub const fn is_dark(&self) -> bool {
        !matches!(self, ThemeMode::Light)
    }

    pub const ALL: [ThemeMode; 6] = [
        ThemeMode::System,
        ThemeMode::Light,
        ThemeMode::DarkCatppuccin,
        ThemeMode::DarkTokyoNight,
        ThemeMode::DarkGruvbox,
        ThemeMode::DarkEverforest,
    ];
}

impl FromStr for ThemeMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "Dark" => Ok(ThemeMode::System),
            "System" => Ok(ThemeMode::System),
            "Light" => Ok(ThemeMode::Light),
            "Catppuccin Mocha" | "Catppuccin" => Ok(ThemeMode::DarkCatppuccin),
            "Tokyo Night" | "TokyoNight" => Ok(ThemeMode::DarkTokyoNight),
            "Gruvbox" => Ok(ThemeMode::DarkGruvbox),
            "Everforest" => Ok(ThemeMode::DarkEverforest),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowGeometry {
    pub size: Option<[f32; 2]>,
    pub position: Option<[f32; 2]>,
    pub maximized: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub first_run_complete: bool,
    pub theme: ThemeMode,
    pub window: WindowGeometry,
    pub last_export_folder: Option<PathBuf>,
    pub last_open_folder: Option<PathBuf>,
    pub recent_files: RecentFiles,
    pub update_check_enabled: bool,
    pub update_notify_disabled: bool,
    pub fast_export: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            first_run_complete: false,
            theme: ThemeMode::default(),
            window: WindowGeometry::default(),
            last_export_folder: None,
            last_open_folder: None,
            recent_files: RecentFiles::new(),
            update_check_enabled: true,
            update_notify_disabled: false,
            fast_export: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::load_from_path(&Self::path())
    }

    pub fn load_from_path(path: &Path) -> Self {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(_) => return Self::default(),
        };

        let mut config = Self::default();
        // Buffer recent-file lines so we can re-insert them in MRU
        // order (position 0 = front) regardless of line order in the
        // settings file. The BTreeMap keeps insertion ordered by key.
        let mut pending_recent: std::collections::BTreeMap<usize, PathBuf> =
            std::collections::BTreeMap::new();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            match key {
                "first_run_complete" => {
                    config.first_run_complete = value.eq_ignore_ascii_case("true");
                }
                "theme" => {
                    if let Ok(theme) = ThemeMode::from_str(value) {
                        config.theme = theme;
                    }
                }
                "window_size" => {
                    config.window.size = parse_pair(value);
                }
                "window_position" => {
                    config.window.position = parse_pair(value);
                }
                "window_maximized" => {
                    config.window.maximized = value.eq_ignore_ascii_case("true");
                }
                "last_export_folder" => {
                    if !value.is_empty() {
                        config.last_export_folder = Some(PathBuf::from(value));
                    }
                }
                "last_open_folder" => {
                    if !value.is_empty() {
                        config.last_open_folder = Some(PathBuf::from(value));
                    }
                }
                key if key.starts_with("recent_") => {
                    // Recent files are persisted as `recent_N=/path` where
                    // N is the original MRU position. Order is restored by
                    // re-inserting in position order. Missing entries
                    // (N exceeds the line count, or duplicate) are
                    // silently dropped.
                    if let Some(index) = key
                        .strip_prefix("recent_")
                        .and_then(|n| n.parse::<usize>().ok())
                        && !value.is_empty()
                    {
                        pending_recent.insert(index, PathBuf::from(value));
                    }
                }
                "update_check_enabled" => {
                    config.update_check_enabled = value.eq_ignore_ascii_case("true");
                }
                "update_notify_disabled" => {
                    config.update_notify_disabled = value.eq_ignore_ascii_case("true");
                }
                "fast_export" => {
                    config.fast_export = value.eq_ignore_ascii_case("true");
                }
                _ => {}
            }
        }
        // Apply recent files in reverse MRU order. `touch()` prepends
        // to the list, so to reconstruct MRU-first ordering we have to
        // apply the lowest-index (most recent) entry LAST. Iterating
        // the BTreeMap in reverse yields the saved positions from
        // oldest to newest.
        for (_index, path) in pending_recent.into_iter().rev() {
            config.recent_files.touch(path);
        }
        config
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> io::Result<()> {
        let mut file = fs::File::create(path)?;
        writeln!(file, "; IMGEditor v2 configuration")?;
        writeln!(
            file,
            "first_run_complete={}",
            if self.first_run_complete {
                "true"
            } else {
                "false"
            }
        )?;
        writeln!(file, "theme={}", self.theme.as_str())?;
        if let Some(size) = self.window.size {
            writeln!(file, "window_size={:.1},{:.1}", size[0], size[1])?;
        }
        if let Some(position) = self.window.position {
            writeln!(
                file,
                "window_position={:.1},{:.1}",
                position[0], position[1]
            )?;
        }
        if self.window.maximized {
            writeln!(file, "window_maximized=true")?;
        }
        if let Some(folder) = &self.last_export_folder {
            writeln!(file, "last_export_folder={}", folder.display())?;
        }
        if let Some(folder) = &self.last_open_folder {
            writeln!(file, "last_open_folder={}", folder.display())?;
        }
        for (index, entry) in self.recent_files.iter() {
            writeln!(file, "recent_{}={}", index, entry.path.display())?;
        }
        writeln!(
            file,
            "update_check_enabled={}",
            if self.update_check_enabled {
                "true"
            } else {
                "false"
            }
        )?;
        writeln!(
            file,
            "update_notify_disabled={}",
            if self.update_notify_disabled {
                "true"
            } else {
                "false"
            }
        )?;
        writeln!(
            file,
            "fast_export={}",
            if self.fast_export { "true" } else { "false" }
        )?;
        Ok(())
    }

    pub fn config_dir() -> PathBuf {
        if let Ok(app_data) = std::env::var("APPDATA") {
            PathBuf::from(app_data).join("IMGEditor")
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("IMGEditor")
        }
    }

    pub fn path() -> PathBuf {
        Self::config_dir().join("settings.ini")
    }
}

fn parse_pair(value: &str) -> Option<[f32; 2]> {
    let mut parts = value.split(',');
    let first = parts.next()?.trim().parse().ok()?;
    let second = parts.next()?.trim().parse().ok()?;
    Some([first, second])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn config_defaults_are_sensible() {
        let config = Config::default();
        assert!(!config.first_run_complete);
        assert_eq!(config.theme, ThemeMode::System);
        assert!(config.window.size.is_none());
        assert!(config.window.position.is_none());
    }

    #[test]
    fn config_round_trip() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("settings.ini");

        let mut original = Config {
            first_run_complete: true,
            theme: ThemeMode::DarkTokyoNight,
            window: WindowGeometry {
                size: Some([1280.0, 800.0]),
                position: Some([100.0, 50.0]),
                maximized: true,
            },
            last_export_folder: Some(PathBuf::from("C:/out")),
            last_open_folder: Some(PathBuf::from("C:/in")),
            recent_files: RecentFiles::new(),
            update_check_enabled: false,
            update_notify_disabled: true,
            fast_export: true,
        };
        let archive_a = temp.path().join("a.img");
        let archive_b = temp.path().join("b.img");
        fs::write(&archive_a, b"a").unwrap();
        fs::write(&archive_b, b"b").unwrap();
        original.recent_files.touch(&archive_a);
        original.recent_files.touch(&archive_b);
        original.save_to_path(&path).unwrap();

        let loaded = Config::load_from_path(&path);
        assert!(loaded.first_run_complete);
        assert_eq!(loaded.theme, ThemeMode::DarkTokyoNight);
        assert_eq!(loaded.window.size, Some([1280.0, 800.0]));
        assert_eq!(loaded.window.position, Some([100.0, 50.0]));
        assert!(loaded.window.maximized);
        assert_eq!(loaded.last_export_folder, Some(PathBuf::from("C:/out")));
        assert_eq!(loaded.last_open_folder, Some(PathBuf::from("C:/in")));
        assert_eq!(loaded.recent_files.len(), 2);
        // MRU-first: b was touched last, so it's at index 0.
        let canonical_b = archive_b.canonicalize().unwrap();
        assert_eq!(
            &loaded.recent_files.iter().next().unwrap().1.path,
            &canonical_b
        );
        assert!(!loaded.update_check_enabled);
        assert!(loaded.fast_export);
    }

    #[test]
    fn config_ignores_unknown_keys() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("settings.ini");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "unknown_key=ignored").unwrap();
        writeln!(file, "theme=Light").unwrap();
        drop(file);

        let loaded = Config::load_from_path(&path);
        assert_eq!(loaded.theme, ThemeMode::Light);
    }

    #[test]
    fn config_handles_missing_file() {
        let loaded = Config::load_from_path(Path::new("does_not_exist.ini"));
        assert_eq!(loaded.theme, ThemeMode::System);
    }

    // ---- RecentFiles tests ----

    #[test]
    fn recent_files_touch_inserts_mru_first() {
        let mut r = RecentFiles::new();
        r.touch("/tmp/a.img");
        r.touch("/tmp/b.img");
        let labels: Vec<_> = r.iter().map(|(_, e)| e.display_name().to_string()).collect();
        // display_name is just the file_name component
        assert_eq!(labels, vec!["b.img", "a.img"]);
    }

    #[test]
    fn recent_files_touch_existing_moves_to_front() {
        let mut r = RecentFiles::new();
        r.touch("/tmp/a.img");
        r.touch("/tmp/b.img");
        r.touch("/tmp/c.img");
        r.touch("/tmp/a.img"); // re-touch
        assert_eq!(r.len(), 3);
        let names: Vec<_> = r.iter().map(|(_, e)| e.display_name().to_string()).collect();
        assert_eq!(names, vec!["a.img", "c.img", "b.img"]);
    }

    #[test]
    fn recent_files_caps_at_max() {
        let mut r = RecentFiles::new();
        for i in 0..(RECENT_FILES_MAX + 5) {
            r.touch(format!("/tmp/file_{i}.img"));
        }
        assert_eq!(r.len(), RECENT_FILES_MAX);
    }

    #[test]
    fn recent_files_remove_drops_entry() {
        let mut r = RecentFiles::new();
        r.touch("/tmp/a.img");
        r.touch("/tmp/b.img");
        r.touch("/tmp/c.img");
        r.remove("/tmp/b.img");
        let names: Vec<_> = r.iter().map(|(_, e)| e.display_name().to_string()).collect();
        assert_eq!(names, vec!["c.img", "a.img"]);
    }

    #[test]
    fn recent_files_filter_existing_skips_missing() {
        let temp = TempDir::new().unwrap();
        let existing = temp.path().join("real.img");
        fs::write(&existing, b"x").unwrap();
        let mut r = RecentFiles::new();
        r.touch(&existing);
        // Second entry: a path that canonicalize can't resolve
        // (parent directory doesn't exist on disk). Falls back to the
        // raw path; iter_existing should still skip it because
        // path.exists() returns false.
        let missing = temp.path().join("does_not_exist_subdir/missing.img");
        r.touch(&missing);
        let existing_only: Vec<_> = r
            .iter_existing()
            .map(|(_, e)| e.path.clone())
            .collect();
        assert_eq!(existing_only.len(), 1);
        // touch() canonicalizes the path (\\?\ prefix on Windows), so
        // compare against the canonicalized form.
        let canonical_existing = existing.canonicalize().unwrap();
        assert_eq!(existing_only[0], canonical_existing);
    }

    #[test]
    fn recent_files_clear_empties_list() {
        let mut r = RecentFiles::new();
        r.touch("/tmp/a.img");
        r.clear();
        assert!(r.is_empty());
    }

    #[test]
    fn recent_files_menu_label_truncates_long_paths() {
        let mut r = RecentFiles::new();
        r.touch("/very/long/path/that/exceeds/the/typical/width/allowed/for/menu/items/cool_game.img");
        let label = r.menu_label(0, 30);
        // Must contain the filename
        assert!(label.contains("cool_game.img"));
        // And be no longer than the cap + a small fudge for the prefix
        assert!(label.len() <= 35, "label too long: {label:?}");
    }

    #[test]
    fn recent_files_persists_through_config() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("settings.ini");
        let a = temp.path().join("a.img");
        let b = temp.path().join("b.img");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();

        let mut cfg = Config::default();
        cfg.recent_files.touch(&a);
        cfg.recent_files.touch(&b);
        cfg.save_to_path(&path).unwrap();

        let loaded = Config::load_from_path(&path);
        assert_eq!(loaded.recent_files.len(), 2);
        // b was touched last → at front
        let first = &loaded.recent_files.iter().next().unwrap().1.path;
        // touch() canonicalizes (\\?\ prefix on Windows), so compare
        // against the canonicalized b.
        let canonical_b = b.canonicalize().unwrap();
        assert_eq!(first, &canonical_b);
    }

    #[test]
    fn recent_files_out_of_order_lines_preserve_mru() {
        // Lines in the settings file may be out of order (e.g. user
        // edited the file manually). The position suffix must drive
        // reconstruction, not line order.
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("settings.ini");
        let a = temp.path().join("a.img");
        let b = temp.path().join("b.img");
        let c = temp.path().join("c.img");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        fs::write(&c, b"c").unwrap();

        let mut file = fs::File::create(&path).unwrap();
        // Intentionally scrambled: 2, 0, 1
        writeln!(file, "recent_2={}", c.display()).unwrap();
        writeln!(file, "recent_0={}", a.display()).unwrap();
        writeln!(file, "recent_1={}", b.display()).unwrap();
        drop(file);

        let loaded = Config::load_from_path(&path);
        let names: Vec<_> = loaded
            .recent_files
            .iter()
            .map(|(_, e)| e.path.clone())
            .collect();
        // touch() canonicalizes the path; compare against canonicalized
        // versions of the original inputs.
        assert_eq!(names, vec![a.canonicalize().unwrap(), b.canonicalize().unwrap(), c.canonicalize().unwrap()]);
    }

    #[test]
    fn theme_mode_from_str_accepts_aliases() {
        assert_eq!(
            "Catppuccin".parse::<ThemeMode>().unwrap(),
            ThemeMode::DarkCatppuccin
        );
        assert_eq!(
            "TokyoNight".parse::<ThemeMode>().unwrap(),
            ThemeMode::DarkTokyoNight
        );
        assert_eq!("Gruvbox".parse::<ThemeMode>().unwrap(), ThemeMode::DarkGruvbox);
    }
}
