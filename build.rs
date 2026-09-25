#[path = "build/i18n.rs"]
mod i18n;

fn main() {
    embed_resource::compile("asset/logo/icon.rc", embed_resource::NONE);
    i18n::generate();
}
