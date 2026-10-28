//! Harness stub: eframe::storage_dir, which paths.rs uses only when adopting
//! data folders from the app's previous names. Tests work in tempdirs, so
//! the stub reports "no such folder".
pub fn storage_dir(_app_name: &str) -> Option<std::path::PathBuf> {
    None
}
