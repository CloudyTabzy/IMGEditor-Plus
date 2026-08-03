//! Semantic Lucide icon helpers used by the desktop UI.
//!
//! Keeping icon selection here gives the interface one visual vocabulary and
//! makes future icon swaps a one-file change.

use iced::widget::Text;
use iced_fonts::lucide;

pub fn new_archive<'a>() -> Text<'a> {
    lucide::file_plus()
}

pub fn open_archive<'a>() -> Text<'a> {
    lucide::folder_open()
}

pub fn save<'a>() -> Text<'a> {
    lucide::save()
}

pub fn import<'a>() -> Text<'a> {
    lucide::download()
}

pub fn export<'a>() -> Text<'a> {
    lucide::upload()
}

pub fn delete<'a>() -> Text<'a> {
    lucide::trash_two()
}

pub fn close<'a>() -> Text<'a> {
    lucide::x()
}

pub fn check<'a>() -> Text<'a> {
    lucide::check()
}

pub fn invert_selection<'a>() -> Text<'a> {
    lucide::refresh_cw()
}

pub fn refresh<'a>() -> Text<'a> {
    lucide::refresh_cw()
}

pub fn sort<'a>() -> Text<'a> {
    lucide::list_filter()
}

pub fn settings<'a>() -> Text<'a> {
    lucide::settings()
}

pub fn help<'a>() -> Text<'a> {
    lucide::info()
}

pub fn copy<'a>() -> Text<'a> {
    lucide::copy()
}

pub fn rename<'a>() -> Text<'a> {
    lucide::pencil()
}

pub fn inspect<'a>() -> Text<'a> {
    lucide::file_search()
}

pub fn search<'a>() -> Text<'a> {
    lucide::search()
}

pub fn external_viewer<'a>() -> Text<'a> {
    lucide::external_link()
}

pub fn model<'a>() -> Text<'a> {
    lucide::r#box()
}

pub fn texture<'a>() -> Text<'a> {
    lucide::image()
}

pub fn archive<'a>() -> Text<'a> {
    lucide::archive()
}

pub fn database<'a>() -> Text<'a> {
    lucide::database()
}

pub fn generic_file<'a>() -> Text<'a> {
    lucide::file()
}

pub fn file_type<'a>(file_name: &str) -> Text<'a> {
    match file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("img") => archive(),
        Some("db") => database(),
        Some("nif" | "dff") => model(),
        Some("nft" | "txd") => texture(),
        _ => generic_file(),
    }
}
