use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    System,
}

impl Theme {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::System => "System",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim() {
            "Light" => Some(Theme::Light),
            "Dark" => Some(Theme::Dark),
            "System" => Some(Theme::System),
            _ => None,
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        match self {
            Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
            Theme::System => {}
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::System
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub first_run_complete: bool,
    pub theme: Theme,
    pub window_size: Option<[f32; 2]>,
    pub window_position: Option<[f32; 2]>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            first_run_complete: false,
            theme: Theme::default(),
            window_size: None,
            window_position: None,
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
                    if let Some(theme) = Theme::from_str(value) {
                        config.theme = theme;
                    }
                }
                "window_size" => {
                    config.window_size = parse_pair(value);
                }
                "window_position" => {
                    config.window_position = parse_pair(value);
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
        writeln!(file, "; IMGEditor configuration")?;
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
        if let Some(size) = self.window_size {
            writeln!(file, "window_size={:.1},{:.1}", size[0], size[1])?;
        }
        if let Some(position) = self.window_position {
            writeln!(
                file,
                "window_position={:.1},{:.1}",
                position[0], position[1]
            )?;
        }
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
        assert_eq!(config.theme, Theme::System);
        assert!(config.window_size.is_none());
        assert!(config.window_position.is_none());
    }

    #[test]
    fn config_round_trip() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("settings.ini");

        let original = Config {
            first_run_complete: true,
            theme: Theme::Dark,
            window_size: Some([1200.0, 800.0]),
            window_position: Some([100.0, 50.0]),
        };
        original.save_to_path(&path).unwrap();

        let loaded = Config::load_from_path(&path);
        assert_eq!(loaded.first_run_complete, true);
        assert_eq!(loaded.theme, Theme::Dark);
        assert_eq!(loaded.window_size, Some([1200.0, 800.0]));
        assert_eq!(loaded.window_position, Some([100.0, 50.0]));
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
        assert_eq!(loaded.theme, Theme::Light);
    }

    #[test]
    fn config_handles_missing_file() {
        let loaded = Config::load_from_path(Path::new("does_not_exist.ini"));
        assert_eq!(loaded.theme, Theme::System);
    }
}
