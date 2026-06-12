use std::path::{Path, PathBuf};

use crate::parser::{ImgParser, ImgVersion, MAX_ENTRY_NAME_BYTES, encode_entry_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub offset: u32,
    pub sector: u32,
    pub file_name: String,
    pub file_name_raw: [u8; MAX_ENTRY_NAME_BYTES],
    pub file_type: String,
    pub source_path: Option<PathBuf>,
    pub imported: bool,
    pub rename: bool,
    pub selected: bool,
}

impl EntryInfo {
    pub fn new(file_name: impl Into<String>) -> Self {
        let file_name = file_name.into();
        let file_name_raw = encode_entry_name(&file_name);
        let file_type = infer_file_type(&file_name).to_string();

        Self {
            offset: 0,
            sector: 0,
            file_name,
            file_name_raw,
            file_type,
            source_path: None,
            imported: false,
            rename: false,
            selected: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgressInfo {
    pub percentage: f32,
    pub cancel: bool,
    pub in_use: bool,
}

#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    pub path: Option<PathBuf>,
    pub file_name: String,
    pub entries: Vec<EntryInfo>,
    pub selected_indices: Vec<usize>,
    pub logs: Vec<String>,
    pub progress: ProgressInfo,
    pub version: ImgVersion,
    pub open: bool,
    pub create_new: bool,
    pub update_search: bool,
}

impl ArchiveInfo {
    pub fn new(file_name: impl Into<String>, create_new: bool, version: ImgVersion) -> Self {
        let mut archive = Self {
            path: None,
            file_name: file_name.into(),
            entries: Vec::new(),
            selected_indices: Vec::new(),
            logs: Vec::new(),
            progress: ProgressInfo::default(),
            version,
            open: true,
            create_new,
            update_search: false,
        };

        archive.add_log("Created archive".to_string());
        archive
    }

    pub fn open(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let version = crate::parser::detect_version(&path);

        let mut archive = Self {
            path: Some(path.clone()),
            file_name: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Untitled")
                .to_string(),
            entries: Vec::new(),
            selected_indices: Vec::new(),
            logs: Vec::new(),
            progress: ProgressInfo::default(),
            version,
            open: true,
            create_new: false,
            update_search: false,
        };

        match version {
            ImgVersion::One => crate::parser::PcV1Parser.open(&mut archive)?,
            ImgVersion::Two => crate::parser::PcV2Parser.open(&mut archive)?,
            ImgVersion::Unknown => crate::parser::UnknownParser.open(&mut archive)?,
        }

        archive.update_selected_list("");
        Ok(archive)
    }

    pub fn add_log(&mut self, message: String) {
        self.logs.push(message);
    }

    pub fn update_selected_list(&mut self, filter: &str) {
        let filter = filter.to_lowercase();
        self.selected_indices.clear();

        for (index, entry) in self.entries.iter().enumerate() {
            if entry.file_name.to_lowercase().contains(&filter) {
                self.selected_indices.push(index);
            }
        }
    }
}

pub fn infer_file_type(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();

    if lower.contains(".dff") {
        "Model".to_string()
    } else if lower.contains(".txd") {
        "Texture".to_string()
    } else if lower.contains(".col") {
        "Collision".to_string()
    } else if lower.contains(".ifp") {
        "Animation".to_string()
    } else if lower.contains(".ipl") {
        "Placement".to_string()
    } else if lower.contains(".ide") {
        "Definition".to_string()
    } else if lower.contains(".dat") {
        "Data".to_string()
    } else {
        Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{} file", ext.to_ascii_lowercase()))
            .unwrap_or_else(|| "file".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_known_types() {
        assert_eq!(infer_file_type("player.dff"), "Model");
        assert_eq!(infer_file_type("PLAYER.TXD"), "Texture");
        assert_eq!(infer_file_type("coll.col"), "Collision");
        assert_eq!(infer_file_type("anim.ifp"), "Animation");
        assert_eq!(infer_file_type("item.ipl"), "Placement");
        assert_eq!(infer_file_type("object.ide"), "Definition");
        assert_eq!(infer_file_type("data.dat"), "Data");
    }

    #[test]
    fn infer_substring_match() {
        assert_eq!(infer_file_type("player.dff.backup"), "Model");
    }

    #[test]
    fn infer_unknown_extension() {
        assert_eq!(infer_file_type("readme.txt"), ".txt file");
    }

    #[test]
    fn infer_no_extension() {
        assert_eq!(infer_file_type("readme"), "file");
    }

    #[test]
    fn entry_info_new_sets_raw_name() {
        let entry = EntryInfo::new("test.dff");
        assert_eq!(entry.file_name, "test.dff");
        assert_eq!(entry.file_type, "Model");
        assert_eq!(&entry.file_name_raw[..8], b"test.dff");
    }

    #[test]
    fn archive_update_selected_list_filters_by_name() {
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("aaa.dff"));
        archive.entries.push(EntryInfo::new("bbb.txd"));
        archive.entries.push(EntryInfo::new("aab.dff"));

        archive.update_selected_list("aa");
        assert_eq!(archive.selected_indices, vec![0, 2]);

        archive.update_selected_list("txd");
        assert_eq!(archive.selected_indices, vec![1]);
    }
}
