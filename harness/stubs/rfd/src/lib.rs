//! Harness stub: the portable check never opens a desktop file dialog.
use std::path::PathBuf;

pub struct FileDialog;

impl FileDialog {
    pub fn new() -> Self {
        Self
    }

    pub fn set_title(self, _title: &str) -> Self {
        self
    }

    pub fn set_file_name(self, _name: &str) -> Self { self }

    pub fn save_file(self) -> Option<PathBuf> { None }

    pub fn pick_folder(self) -> Option<PathBuf> { None }

    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        None
    }
}
