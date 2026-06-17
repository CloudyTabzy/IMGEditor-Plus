use iced::Theme;

use crate::config::ThemeMode;

pub const APP_NAME: &str = "IMG Editor";

/// Resolve the runtime [`Theme`] from the user-configured [`ThemeMode`],
/// falling back to system preference when `ThemeMode::System` is selected.
///
/// The `dark_light` crate is already a transitive dependency of Iced, so we
/// reuse the same probe the framework uses internally to stay consistent.
pub fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::System => {
            match dark_light::detect() {
                Ok(dark_light::Mode::Dark) => Theme::Dark,
                _ => Theme::Light,
            }
        }
        ThemeMode::Light => Theme::Light,
        ThemeMode::DarkCatppuccin => Theme::CatppuccinMocha,
        ThemeMode::DarkTokyoNight => Theme::TokyoNight,
        ThemeMode::DarkGruvbox => Theme::GruvboxDark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_modes_resolve_correctly() {
        assert!(matches!(resolve_theme(ThemeMode::Light), Theme::Light));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkCatppuccin),
            Theme::CatppuccinMocha
        ));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkTokyoNight),
            Theme::TokyoNight
        ));
        assert!(matches!(resolve_theme(ThemeMode::DarkGruvbox), Theme::GruvboxDark));
    }
}
