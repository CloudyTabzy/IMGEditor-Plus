//! Export embedded textures from a NIF/NFT to disk.
//!
//! Mirrors the "Save texture" / "Export Textures" workflow from NifSkope
//! (see niftools/nifskope issue #183). Given a NIF basename, resolves the
//! matching `.nft` via the IDE->NFT mapping, then writes every embedded
//! texture to the chosen destination directory using the source path's
//! original basename and extension.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::inspector::texture::{IdeMap, NftCatalog, TextureEntry, resolve_textures_for_nif};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub written: usize,
    pub skipped_no_data: usize,
    pub failures: Vec<(String, String)>,
}

impl ExportReport {
    pub fn is_success(&self) -> bool {
        self.failures.is_empty() && self.written > 0
    }

    pub fn summary(&self) -> String {
        if self.failures.is_empty() {
            if self.written == 0 {
                "No embedded textures found".to_string()
            } else {
                format!("Exported {} embedded texture(s)", self.written)
            }
        } else {
            format!(
                "Exported {} texture(s), {} failure(s)",
                self.written,
                self.failures.len()
            )
        }
    }
}

fn output_extension(source_path: &str) -> String {
    let lower = source_path.to_lowercase();
    for ext in &[".tga", ".dds", ".png", ".bmp", ".jpg", ".jpeg"] {
        if lower.ends_with(ext) {
            let len = source_path.len();
            let ext_start = len - ext.len();
            return source_path[ext_start..].to_string();
        }
    }
    ".bin".to_string()
}

fn output_filename(key: &str, entry: &TextureEntry) -> String {
    let basename = Path::new(&entry.source_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(key)
        .to_string();
    if Path::new(&basename).extension().is_some() {
        basename
    } else {
        format!("{}{}", basename, output_extension(&entry.source_path))
    }
}

fn write_entry(dest_dir: &Path, name: &str, entry: &TextureEntry) -> io::Result<()> {
    let Some(pixel_data) = entry.pixel_data.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no embedded pixel data",
        ));
    };
    let dest = dest_dir.join(name);
    fs::write(dest, pixel_data)
}

pub fn export_embedded_textures(
    nif_basename: &str,
    ide_map: &IdeMap,
    dest_dir: &Path,
) -> Result<ExportReport, String> {
    let catalog: NftCatalog = resolve_textures_for_nif(nif_basename, ide_map)
        .ok_or_else(|| format!("No NFT found for '{nif_basename}'"))?;

    let mut report = ExportReport::default();

    for (key, entry) in &catalog.entries {
        let name = output_filename(key, entry);
        match write_entry(dest_dir, &name, entry) {
            Ok(()) => report.written += 1,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                report.skipped_no_data += 1;
            }
            Err(e) => {
                report.failures.push((name, e.to_string()));
            }
        }
    }

    Ok(report)
}

pub fn unique_destination(dest_dir: &Path, name: &str) -> PathBuf {
    let candidate = dest_dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for n in 1..=u32::MAX {
        let suffix = if ext.is_empty() {
            format!("_{n}")
        } else {
            format!("_{n}.{ext}")
        };
        let candidate_name = format!("{stem}{suffix}");
        let candidate_path = dest_dir.join(&candidate_name);
        if !candidate_path.exists() {
            return candidate_path;
        }
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: &str, pixels: Option<Vec<u8>>) -> TextureEntry {
        TextureEntry {
            source_path: source.to_string(),
            pixel_data: pixels,
        }
    }

    #[test]
    fn output_extension_known_types() {
        assert_eq!(output_extension("X:\\Path\\foo.tga"), ".tga");
        assert_eq!(output_extension("X:\\Path\\FOO.DDS"), ".DDS");
        assert_eq!(output_extension("X:\\Path\\bar.png"), ".png");
        assert_eq!(output_extension("X:\\Path\\baz.bmp"), ".bmp");
        assert_eq!(output_extension("X:\\Path\\noext"), ".bin");
        assert_eq!(output_extension(""), ".bin");
    }

    #[test]
    fn output_filename_prefers_source_basename() {
        let e = entry("Z:\\Exp\\Textures\\PO00_guts_d.tga", Some(vec![0u8; 4]));
        assert_eq!(output_filename("po00_guts_d", &e), "PO00_guts_d.tga");
    }

    #[test]
    fn output_filename_falls_back_to_key() {
        let e = entry("", Some(vec![0u8; 4]));
        assert_eq!(output_filename("sc06_steelrust_d", &e), "sc06_steelrust_d.bin");
    }

    #[test]
    fn report_summary() {
        let r = ExportReport {
            written: 4,
            skipped_no_data: 1,
            failures: vec![],
        };
        assert_eq!(r.summary(), "Exported 4 embedded texture(s)");
        assert!(r.is_success());

        let empty = ExportReport::default();
        assert_eq!(empty.summary(), "No embedded textures found");
        assert!(!empty.is_success());

        let failed = ExportReport {
            written: 1,
            skipped_no_data: 0,
            failures: vec![("x.tga".to_string(), "boom".to_string())],
        };
        assert_eq!(failed.summary(), "Exported 1 texture(s), 1 failure(s)");
        assert!(!failed.is_success());
    }

    #[test]
    fn unique_destination_no_clash() {
        let tmp = std::env::temp_dir().join(format!(
            "imgeditor_test_unique_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&tmp).unwrap();
        let path = unique_destination(&tmp, "fresh.tga");
        assert_eq!(path, tmp.join("fresh.tga"));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn unique_destination_with_clash() {
        let tmp = std::env::temp_dir().join(format!(
            "imgeditor_test_unique_clash_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("taken.tga"), b"x").unwrap();
        let path = unique_destination(&tmp, "taken.tga");
        assert_eq!(path, tmp.join("taken_1.tga"));
        fs::remove_dir_all(&tmp).ok();
    }
}
