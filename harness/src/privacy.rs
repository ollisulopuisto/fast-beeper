//! Harness stub: the two enums, verbatim from solmu src/privacy.rs,
//! without the wacore wire conversions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivacyKind {
    LastSeen,
    Online,
    Profile,
    About,
    GroupAdd,
    ReadReceipts,
    CallAdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivacyChoice {
    Everyone,
    Contacts,
    Except,
    Nobody,
    SameAsLastSeen,
    Known,
}

/// Harness stub: account.rs only holds a default Snapshot.
/// Real: solmu src/privacy.rs.
#[derive(Clone, Debug, Default)]
pub struct Snapshot;
