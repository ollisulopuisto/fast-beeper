//! Harness stub: opaque payloads plus the key-copy signature paths.rs
//! names. Real: zapfast src/archive.rs (rusqlite + keyring).
#[derive(Clone, Debug, PartialEq)]
pub struct Pinned;

#[derive(Clone, Debug, PartialEq)]
pub struct PollVote;

#[derive(Clone, Debug, PartialEq)]
pub struct Starred;

pub fn copy_archive_key(
    _old_archive: &std::path::Path,
    _new_archive: &std::path::Path,
) -> std::io::Result<()> {
    Ok(())
}

/// Explicit leaf stub: encrypted storage is exercised in the real GUI library,
/// not replaced by a pretend durable map here.
pub struct Archive;
impl Archive {
    pub fn open(_: &std::path::Path) -> Result<Self, String> { Err("OS-keyring archive unavailable in adapter-only harness".into()) }
    pub fn drafts(&self) -> Result<Vec<(String,String)>, String> { Err("archive stub".into()) }
    pub fn set_draft(&self, _: &str, _: &str, _: i64) -> Result<(),String> { Err("archive stub".into()) }
}
