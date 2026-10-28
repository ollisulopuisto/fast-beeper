//! Harness stub. Real: zapfast src/app.rs (egui app state).
pub type AttachmentId = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerMention {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
}

/// Harness stub: account.rs stores these opaquely. Real: zapfast src/app.rs.
pub struct Conversation;

/// Harness stub: account.rs stores these opaquely. Real: zapfast src/app.rs.
pub struct AttachmentDraft;

/// Harness stub: account.rs stores these opaquely. Real: zapfast src/app.rs.
pub struct Presence;
