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

// GitHub Primer "dark default": canvas #0D1117, fg #E6EDF3, blue accent.
const GITHUB_DARK_PALETTE: iced::theme::Palette = iced::theme::Palette {
    background: iced::color!(0x0D1117),
    text: iced::color!(0xE6EDF3),
    primary: iced::color!(0x2F81F7),
    success: iced::color!(0x3FB950),
    warning: iced::color!(0xD29922),
    danger: iced::color!(0xF85149),
};

// Ayu Dark (ayutheme.com) tuned toward a deep terracotta-orange accent:
// near-black navy canvas #0B0E14, panel #131722, warm-grey fg, and the
// signature orange pulled slightly earthier (#F29A4B vs. #FF9940).
const AYU_DARK_PALETTE: iced::theme::Palette = iced::theme::Palette {
    background: iced::color!(0x0B0E14),
    text: iced::color!(0xC3C6CE),
    primary: iced::color!(0xF29A4B),
    success: iced::color!(0xAAD94C),
    warning: iced::color!(0xE6B450),
    danger: iced::color!(0xF07178),
};

pub fn light_theme() -> Theme {
    Theme::custom("Light", LIGHT_OCEAN_PALETTE)
}

pub fn everforest_theme() -> Theme {
    Theme::custom("Everforest", EVERFOREST_PALETTE)
}

pub fn github_dark_theme() -> Theme {
    Theme::custom("GitHub Dark", GITHUB_DARK_PALETTE)
}

pub fn ayu_dark_theme() -> Theme {
    Theme::custom("Ayu Dark", AYU_DARK_PALETTE)
}

pub fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::System => Theme::Dark,
        ThemeMode::Light => light_theme(),
        ThemeMode::DarkCatppuccin => Theme::CatppuccinMocha,
        ThemeMode::DarkTokyoNight => Theme::TokyoNight,
        ThemeMode::DarkGruvbox => Theme::GruvboxDark,
        ThemeMode::DarkEverforest => everforest_theme(),
        ThemeMode::DarkGithub => github_dark_theme(),
        ThemeMode::DarkAyu => ayu_dark_theme(),
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
        assert!(matches!(
            resolve_theme(ThemeMode::DarkGruvbox),
            Theme::GruvboxDark
        ));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkEverforest),
            Theme::Custom(_)
        ));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkGithub),
            Theme::Custom(_)
        ));
        assert!(matches!(
            resolve_theme(ThemeMode::DarkAyu),
            Theme::Custom(_)
        ));
    }
}
