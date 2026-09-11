use std::path::Path;

use anyhow::Result;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{
    ImgParser, ImgVersion, V1ByteOrder, export_entry_to_file, import_entry,
};
use crate::parser::pc_v1::PcV1Parser;

#[derive(Debug, Default, Clone, Copy)]
pub struct Xbox360Parser;

impl ImgParser for Xbox360Parser {
    fn open(&self, archive: &mut ArchiveInfo) -> Result<()> {
        PcV1Parser.open_with_endian(archive, V1ByteOrder::Big)
    }

    fn export_entry(
        &self,
        archive: &ArchiveInfo,
        entry: &EntryInfo,
        output_path: &Path,
    ) -> Result<()> {
        export_entry_to_file(archive, entry, output_path)
    }

    fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> Result<()> {
        import_entry(archive, path, replace)
    }

    fn save(
        &self,
        archive: &mut ArchiveInfo,
        output_path: &Path,
        remove_existing: bool,
    ) -> Result<()> {
        PcV1Parser.save_with_endian(
            archive,
            output_path,
            remove_existing,
            V1ByteOrder::Big,
            ImgVersion::Xbox360,
        )
    }

    fn version_text(&self) -> &'static str {
        "Xbox 360 IMG v1"
    }

    fn is_valid(&self, path: &Path) -> bool {
        PcV1Parser.is_valid_with_endian(path, V1ByteOrder::Big)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::archive::ArchiveInfo;
    use crate::parser::{SECTOR_SIZE, detect_version, read_entry_data, sector_rounded_size};

    fn write_record(output: &mut Vec<u8>, offset: u32, sectors: u32, name: &str) {
        output.extend_from_slice(&offset.to_be_bytes());
        output.extend_from_slice(&sectors.to_be_bytes());

        let name_bytes = name.as_bytes();
        assert!(name_bytes.len() <= crate::parser::MAX_ENTRY_NAME_BYTES);
        let mut raw = [0u8; crate::parser::MAX_ENTRY_NAME_BYTES];
        raw[..name_bytes.len()].copy_from_slice(name_bytes);
        output.extend_from_slice(&raw);
    }

    fn create_archive(dir: &std::path::Path) -> std::path::PathBuf {
        let img_path = dir.join("scripts.img");
        let dir_path = dir.join("scripts.dir");
        let first = vec![b'A'; SECTOR_SIZE as usize];
        let second = vec![b'B'; (SECTOR_SIZE * 2) as usize];

        let mut img = std::fs::File::create(&img_path).unwrap();
        img.write_all(&first).unwrap();
        img.write_all(&second).unwrap();
        let mut directory = Vec::new();
        write_record(&mut directory, 0, 1, "first.lur");
        write_record(&mut directory, 1, 2, "second.lur");
        std::fs::write(dir_path, directory).unwrap();
        img_path
    }

    #[test]
    fn detects_and_reads_big_endian_archive() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_archive(dir.path());

        assert_eq!(detect_version(&img_path), ImgVersion::Xbox360);
        let archive = ArchiveInfo::open(&img_path).unwrap();

        assert_eq!(archive.version, ImgVersion::Xbox360);
        assert_eq!(archive.entries.len(), 2);
        assert_eq!(archive.entries[0].offset, 0);
        assert_eq!(archive.entries[0].sector, 1);
        assert_eq!(archive.entries[1].offset, 1);
        assert_eq!(archive.entries[1].sector, 2);
        assert_eq!(read_entry_data(&archive, &archive.entries[0]).unwrap(), vec![b'A'; 2048]);
        assert_eq!(
            read_entry_data(&archive, &archive.entries[1]).unwrap(),
            vec![b'B'; 4096]
        );
    }

    #[test]
    fn saves_big_endian_directory_and_reopens_as_xbox() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_archive(dir.path());
        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        let save_path = dir.path().join("saved.img");

        Xbox360Parser.save(&mut archive, &save_path, false).unwrap();

        let directory = std::fs::read(dir.path().join("saved.dir")).unwrap();
        assert_eq!(&directory[..8], &[0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(detect_version(&save_path), ImgVersion::Xbox360);

        let reopened = ArchiveInfo::open(&save_path).unwrap();
        assert_eq!(reopened.version, ImgVersion::Xbox360);
        assert_eq!(reopened.entries[1].file_name, "second.lur");
        assert_eq!(
            read_entry_data(&reopened, &reopened.entries[1])
                .unwrap()
                .len() as u64,
            sector_rounded_size(4096)
        );
    }

    #[test]
    fn validates_supplied_bully_xbox360_archive_when_present() {
        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };
        let img_path = root.join("Bully script img xbox 360/Scripts.img");
        let dir_path = root.join("Bully script img xbox 360/Scripts.dir");
        if !img_path.is_file() || !dir_path.is_file() {
            return;
        }

        assert_eq!(detect_version(&dir_path), ImgVersion::Xbox360);
        let archive = ArchiveInfo::open(&dir_path).unwrap();
        assert_eq!(archive.version, ImgVersion::Xbox360);
        assert!(!archive.entries.is_empty());

        let sample_indices = [0, archive.entries.len() / 2, archive.entries.len() - 1];
        let samples = sample_indices
            .into_iter()
            .map(|index| {
                (
                    index,
                    archive.entries[index].file_name.clone(),
                    archive.entries[index].file_name_raw,
                    archive.entries[index].sector,
                    read_entry_data(&archive, &archive.entries[index]).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        let temp = tempfile::tempdir().unwrap();
        let saved_path = temp.path().join("Scripts.img");
        let mut to_save = archive;
        Xbox360Parser
            .save(&mut to_save, &saved_path, false)
            .unwrap();
        let reopened = ArchiveInfo::open(&saved_path).unwrap();
        assert_eq!(reopened.version, ImgVersion::Xbox360);
        assert_eq!(reopened.entries.len(), to_save.entries.len());

        for (index, name, raw, sector, data) in samples {
            let actual = &reopened.entries[index];
            assert_eq!(actual.file_name, name);
            assert_eq!(actual.file_name_raw, raw);
            assert_eq!(actual.sector, sector);
            assert_eq!(read_entry_data(&reopened, actual).unwrap(), data);
        }
    }

    #[test]
    fn imports_full_24_byte_names_for_xbox_archives() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_archive(dir.path());
        let import_name = "PunishmentSystemHack.lur";
        assert_eq!(import_name.len(), 24);
        let import_path = dir.path().join(import_name);
        std::fs::write(&import_path, b"compiled luap").unwrap();

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        Xbox360Parser::import_entry(&mut archive, &import_path, false).unwrap();

        let imported = archive.entries.last().unwrap();
        assert_eq!(imported.file_name, import_name);
        let mut expected = [0u8; crate::parser::MAX_ENTRY_NAME_BYTES];
        expected.copy_from_slice(import_name.as_bytes());
        assert_eq!(imported.file_name_raw, expected);
    }
}
