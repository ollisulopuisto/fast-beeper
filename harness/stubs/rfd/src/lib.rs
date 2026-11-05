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

    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        None
    }
}
