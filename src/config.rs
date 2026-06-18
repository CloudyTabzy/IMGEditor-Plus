use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    DarkCatppuccin,
    DarkTokyoNight,
    DarkGruvbox,
    DarkEverforest,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::System
    }
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

        let original = Config {
            first_run_complete: true,
            theme: ThemeMode::DarkTokyoNight,
            window: WindowGeometry {
                size: Some([1280.0, 800.0]),
                position: Some([100.0, 50.0]),
                maximized: true,
            },
            last_export_folder: Some(PathBuf::from("C:/out")),
            last_open_folder: Some(PathBuf::from("C:/in")),
            update_check_enabled: false,
            update_notify_disabled: true,
            fast_export: true,
        };
        original.save_to_path(&path).unwrap();

        let loaded = Config::load_from_path(&path);
        assert!(loaded.first_run_complete);
        assert_eq!(loaded.theme, ThemeMode::DarkTokyoNight);
        assert_eq!(loaded.window.size, Some([1280.0, 800.0]));
        assert_eq!(loaded.window.position, Some([100.0, 50.0]));
        assert!(loaded.window.maximized);
        assert_eq!(loaded.last_export_folder, Some(PathBuf::from("C:/out")));
        assert_eq!(loaded.last_open_folder, Some(PathBuf::from("C:/in")));
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
