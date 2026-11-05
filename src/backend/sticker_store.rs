//! Sticker files on disk: saved stickers and sticker packs.

/// The file's content hash, the name stickers carry on disk so the same
/// picture is one file.
pub fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
