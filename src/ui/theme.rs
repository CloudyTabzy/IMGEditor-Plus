use iced::Theme;

use crate::config::ThemeMode;

pub const APP_NAME: &str = "IMG Editor Plus";

const EVERFOREST_PALETTE: iced::theme::Palette = iced::theme::Palette {
    background: iced::color!(0x2D353B),
    text: iced::color!(0xD3C6AA),
    primary: iced::color!(0xA7C080),
    success: iced::color!(0x83C092),
    warning: iced::color!(0xDBBC7F),
    danger: iced::color!(0xE67E80),
};

const LIGHT_OCEAN_PALETTE: iced::theme::Palette = iced::theme::Palette {
    background: iced::color!(0xFFFFFF),
    text: iced::color!(0x1F2937),
    primary: iced::color!(0x55E2E9),
    success: iced::color!(0x10B981),
    warning: iced::color!(0xF59E0B),
    danger: iced::color!(0xEF4444),
};

pub fn light_theme() -> Theme {
    Theme::custom("Light", LIGHT_OCEAN_PALETTE)
}

pub fn everforest_theme() -> Theme {
    Theme::custom("Everforest", EVERFOREST_PALETTE)
}

pub fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::System => Theme::Dark,
        ThemeMode::Light => light_theme(),
        ThemeMode::DarkCatppuccin => Theme::CatppuccinMocha,
        ThemeMode::DarkTokyoNight => Theme::TokyoNight,
        ThemeMode::DarkGruvbox => Theme::GruvboxDark,
        ThemeMode::DarkEverforest => everforest_theme(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_modes_resolve_correctly() {
        assert!(matches!(resolve_theme(ThemeMode::Light), Theme::Custom(_)));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkCatppuccin),
            Theme::CatppuccinMocha
        ));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkTokyoNight),
            Theme::TokyoNight
        ));
        assert!(matches!(resolve_theme(ThemeMode::DarkGruvbox), Theme::GruvboxDark));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkEverforest),
            Theme::Custom(_)
        ));
    }
}
