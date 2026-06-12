use std::path::Path;

use crate::archive::{ArchiveInfo, EntryInfo};

pub trait ImgParser {
    fn open(&self, archive: &mut ArchiveInfo) -> anyhow::Result<()>;
    fn export_entry(
        &self,
        archive: &ArchiveInfo,
        entry: &EntryInfo,
        output_path: &Path,
    ) -> anyhow::Result<()>;
    fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> anyhow::Result<()>;
    fn save(&self, archive: &mut ArchiveInfo, output_path: &Path) -> anyhow::Result<()>;
    fn version_text(&self) -> &'static str;
    fn is_valid(&self, path: &Path) -> bool;
}
