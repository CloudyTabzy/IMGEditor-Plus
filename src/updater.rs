pub struct Updater;

impl Updater {
    pub fn check_update(&self) {}
    pub fn is_update_available(&self) -> bool {
        false
    }
    pub fn latest_version(&self) -> Option<String> {
        None
    }
}
