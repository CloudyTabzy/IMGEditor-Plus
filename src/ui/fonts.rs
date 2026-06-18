use iced::font::{Family, Weight};
use iced::widget::Text;

pub const INTER_FONT_BYTES: &[u8] = include_bytes!("../../asset/inter/Inter-VariableFont_opsz,wght.ttf");

pub const BRICOLAGE_DISPLAY_FONT_BYTES: &[u8] =
    include_bytes!("../../asset/Bricolage_Grotesque/static/BricolageGrotesque-ExtraBold.ttf");

pub const INTER: iced::Font = iced::Font {
    family: Family::Name("Inter"),
    weight: Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const INTER_MEDIUM: iced::Font = iced::Font {
    family: Family::Name("Inter"),
    weight: Weight::Medium,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const INTER_SEMIBOLD: iced::Font = iced::Font {
    family: Family::Name("Inter"),
    weight: Weight::Semibold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const INTER_BOLD: iced::Font = iced::Font {
    family: Family::Name("Inter"),
    weight: Weight::Bold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const INTER_EXTRA_BOLD: iced::Font = iced::Font {
    family: Family::Name("Inter"),
    weight: Weight::ExtraBold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const BRICOLAGE_DISPLAY: iced::Font = iced::Font {
    family: Family::Name("Bricolage Grotesque"),
    weight: Weight::ExtraBold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub fn display<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into(), 18.0, BRICOLAGE_DISPLAY)
}

pub fn header<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into(), 14.0, INTER_SEMIBOLD)
}

pub fn strong<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into(), 14.0, INTER_BOLD)
}

pub fn body<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into(), 14.0, INTER)
}

pub fn caption<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into(), 12.0, INTER)
}

pub fn body_monospace<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into(), 13.0, iced::Font::MONOSPACE)
}

fn text<'a>(label: String, size: f32, font: iced::Font) -> Text<'a> {
    iced::widget::text(label).size(size).font(font)
}
