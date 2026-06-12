use std::path::Path;

use anyhow::Result;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::ImgParser;

#[derive(Debug, Default, Clone, Copy)]
pub struct UnknownParser;

impl ImgParser for UnknownParser {
    fn open(&self, _archive: &mut ArchiveInfo) -> Result<()> {
        Ok(())
    }

    fn export_entry(
        &self,
        _archive: &ArchiveInfo,
        _entry: &EntryInfo,
        _output_path: &Path,
    ) -> Result<()> {
        Ok(())
    }

    fn import_entry(_archive: &mut ArchiveInfo, _path: &Path, _replace: bool) -> Result<()> {
        Ok(())
    }

    fn save(
        &self,
        _archive: &mut ArchiveInfo,
        _output_path: &Path,
        _remove_existing: bool,
    ) -> Result<()> {
        Ok(())
    }

    fn version_text(&self) -> &'static str {
        "Unknown"
    }

    fn is_valid(&self, _path: &Path) -> bool {
        false
    }
}
