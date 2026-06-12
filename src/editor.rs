use anyhow::Result;

use crate::ui;

pub struct Editor {
    archives: Vec<crate::archive::ArchiveInfo>,
    selected_archive: Option<usize>,
    selected_entry: Option<usize>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            archives: Vec::new(),
            selected_archive: None,
            selected_entry: None,
        }
    }

    pub fn run() -> Result<()> {
        ui::application::run(ui::renderer::MainWindow::default())
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        Ok(())
    }

    pub fn add_archive(&mut self, archive: crate::archive::ArchiveInfo) {
        self.archives.push(archive);
        self.selected_archive = Some(self.archives.len() - 1);
    }

    pub fn open_archive(&mut self, path: impl Into<std::path::PathBuf>) -> Result<()> {
        let archive = crate::archive::ArchiveInfo::open(path)?;
        self.add_archive(archive);
        Ok(())
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
