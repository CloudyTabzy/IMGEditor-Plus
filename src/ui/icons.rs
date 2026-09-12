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

pub fn pack<'a>() -> Text<'a> {
    lucide::archive()
}

pub fn chevrons_up<'a>() -> Text<'a> {
    lucide::chevrons_up()
}

pub fn chevrons_down<'a>() -> Text<'a> {
    lucide::chevrons_down()
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

pub fn shield_check<'a>() -> Text<'a> {
    lucide::shield_check()
}

/// Validator legend: engine-native format.
pub fn verdict_native<'a>() -> Text<'a> {
    lucide::shield_check()
}

/// Validator legend: supported / losslessly convertible.
pub fn verdict_convert<'a>() -> Text<'a> {
    lucide::refresh_cw()
}

/// Validator legend: lossy conversion required.
pub fn verdict_lossy<'a>() -> Text<'a> {
    lucide::info()
}

/// Validator legend: unknown (no evidence either way).
pub fn verdict_unknown<'a>() -> Text<'a> {
    lucide::file_search()
}

/// Validator legend: the target engine cannot consume this form.
pub fn verdict_incompatible<'a>() -> Text<'a> {
    lucide::x()
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

pub fn animation<'a>() -> Text<'a> {
    lucide::activity()
}

pub fn play<'a>() -> Text<'a> {
    lucide::play()
}

pub fn pause<'a>() -> Text<'a> {
    lucide::pause()
}

pub fn skip_back<'a>() -> Text<'a> {
    lucide::skip_back()
}

pub fn skip_forward<'a>() -> Text<'a> {
    lucide::skip_forward()
}

pub fn step_back<'a>() -> Text<'a> {
    lucide::chevron_left()
}

pub fn step_forward<'a>() -> Text<'a> {
    lucide::chevron_right()
}

pub fn repeat<'a>() -> Text<'a> {
    lucide::repeat()
}

pub fn repeat_once<'a>() -> Text<'a> {
    lucide::repeat_one()
}

pub fn person<'a>() -> Text<'a> {
    lucide::person_standing()
}

pub fn route<'a>() -> Text<'a> {
    lucide::map_pin()
}

pub fn crosshair<'a>() -> Text<'a> {
    lucide::crosshair()
}

pub fn eye<'a>() -> Text<'a> {
    lucide::eye()
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
