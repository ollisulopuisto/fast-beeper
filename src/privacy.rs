//! WhatsApp account privacy: last seen, online, profile photo, About, groups,
//! read receipts, and calls. The values live on the phone, not in
//! `settings.json`; Solmu reads them on connect and writes one category at a
//! time.
//!
//! "My contacts except…" is shown when the account holds it, but not offered:
//! the people it excludes are edited on the phone.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::i18n::{Locale, gettext, pgettext};

/// Account privacy category shown in Settings.
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

impl PrivacyKind {
    /// Every category, in the phone's order.
    pub const ALL: [Self; 7] = [
        Self::LastSeen,
        Self::Online,
        Self::Profile,
        Self::About,
        Self::GroupAdd,
        Self::ReadReceipts,
        Self::CallAdd,
    ];

    pub fn label(self, locale: Locale) -> Cow<'static, str> {
        match self {
            Self::LastSeen => pgettext(locale, "privacy", "Last seen"),
            Self::Online => pgettext(locale, "privacy", "Online"),
            Self::Profile => pgettext(locale, "privacy", "Profile photo"),
            Self::About => pgettext(locale, "privacy", "About"),
            Self::GroupAdd => pgettext(locale, "privacy", "Groups"),
            Self::ReadReceipts => pgettext(locale, "privacy", "Read receipts"),
            Self::CallAdd => pgettext(locale, "privacy", "Calls"),
        }
    }

    pub fn hint(self, locale: Locale) -> Cow<'static, str> {
        match self {
            // The label and the choices say the rest.
            Self::LastSeen | Self::Online | Self::Profile | Self::About | Self::CallAdd => {
                Cow::Borrowed("")
            }
            Self::GroupAdd => gettext(locale, "Who can add you to groups."),
            Self::ReadReceipts => {
                gettext(locale, "For your whole account. Groups always send them.")
            }
        }
    }

    /// The values offered in the picker, which are the ones the phone offers.
    /// The account can hold others (an Except list, or an older "Nobody" for
    /// groups); those are shown but not offered.
    pub fn choices(self) -> &'static [PrivacyChoice] {
        match self {
            Self::LastSeen | Self::Profile | Self::About => &[
                PrivacyChoice::Everyone,
                PrivacyChoice::Contacts,
                PrivacyChoice::Nobody,
            ],
            Self::GroupAdd => &[PrivacyChoice::Everyone, PrivacyChoice::Contacts],
            Self::Online => &[PrivacyChoice::Everyone, PrivacyChoice::SameAsLastSeen],
            Self::ReadReceipts => &[PrivacyChoice::Everyone, PrivacyChoice::Nobody],
            Self::CallAdd => &[PrivacyChoice::Everyone, PrivacyChoice::Known],
        }
    }

    /// A stable name for widget ids.
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::LastSeen => "last",
            Self::Online => "online",
            Self::Profile => "profile",
            Self::About => "status",
            Self::GroupAdd => "groupadd",
            Self::ReadReceipts => "readreceipts",
            Self::CallAdd => "calladd",
        }
    }
}

/// One account privacy value. Not every value is valid for every category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivacyChoice {
    Everyone,
    Contacts,
    /// My contacts except a list the phone edits.
    Except,
    Nobody,
    SameAsLastSeen,
    /// Calls: only known callers ring.
    Known,
}

impl PrivacyChoice {
    pub fn label(self, locale: Locale) -> Cow<'static, str> {
        match self {
            Self::Everyone => pgettext(locale, "privacy", "Everyone"),
            Self::Contacts => pgettext(locale, "privacy", "My contacts"),
            Self::Except => pgettext(locale, "privacy", "My contacts except…"),
            Self::Nobody => pgettext(locale, "privacy", "Nobody"),
            Self::SameAsLastSeen => pgettext(locale, "privacy", "Same as last seen"),
            Self::Known => pgettext(locale, "privacy", "Silence unknown callers"),
        }
    }
}

/// The account's privacy as the phone last reported it, with the writes still
/// in flight applied on top.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    values: HashMap<PrivacyKind, PrivacyChoice>,
    /// The confirmed value behind each write in flight, restored if it fails.
    revert: HashMap<PrivacyKind, PrivacyChoice>,
    /// The last fetch failed; the values shown may be stale.
    pub fetch_failed: bool,
    /// A fetch has succeeded since linking.
    pub loaded: bool,
}

impl Snapshot {
    pub fn get(&self, kind: PrivacyKind) -> Option<PrivacyChoice> {
        self.values.get(&kind).copied()
    }

    pub fn pending(&self, kind: PrivacyKind) -> bool {
        self.revert.contains_key(&kind)
    }

    /// Whether the rows can be edited: the phone's values are loaded and the
    /// last fetch did not fail.
    pub fn editable(&self) -> bool {
        self.loaded && !self.fetch_failed
    }

    /// Takes a fetched snapshot. A failed fetch keeps what was shown and
    /// marks it stale. A write still in flight keeps its value, and the
    /// fetched one becomes what a failure restores.
    pub fn apply_fetch(&mut self, values: Vec<(PrivacyKind, PrivacyChoice)>, failed: bool) {
        self.fetch_failed = failed;
        if failed {
            return;
        }
        let mut fresh: HashMap<_, _> = values.into_iter().collect();
        for (kind, confirmed) in &mut self.revert {
            if let Some(fetched) = fresh.get(kind) {
                *confirmed = *fetched;
            }
            if let Some(optimistic) = self.values.get(kind) {
                fresh.insert(*kind, *optimistic);
            }
        }
        self.values = fresh;
        self.loaded = true;
    }

    /// Shows `choice` at once and remembers what to restore. Returns false
    /// when nothing should be written: the category is unknown, already
    /// holds the value, or has a write in flight.
    pub fn begin_set(&mut self, kind: PrivacyKind, choice: PrivacyChoice) -> bool {
        let Some(current) = self.get(kind) else {
            return false;
        };
        if current == choice || self.pending(kind) || !kind.choices().contains(&choice) {
            return false;
        }
        self.revert.insert(kind, current);
        self.values.insert(kind, choice);
        true
    }

    /// The phone confirmed a write. Returns false for a confirmation nothing
    /// here was waiting for, such as one from before an unlink.
    pub fn finish_set(&mut self, kind: PrivacyKind) -> bool {
        self.revert.remove(&kind).is_some()
    }

    /// The phone refused a write: the confirmed value comes back.
    pub fn fail_set(&mut self, kind: PrivacyKind) {
        if let Some(confirmed) = self.revert.remove(&kind) {
            self.values.insert(kind, confirmed);
        }
    }

    /// Sample values for the demo Settings page.
    pub fn demo() -> Self {
        let mut snapshot = Self::default();
        snapshot.apply_fetch(
            vec![
                (PrivacyKind::LastSeen, PrivacyChoice::Except),
                (PrivacyKind::Online, PrivacyChoice::SameAsLastSeen),
                (PrivacyKind::Profile, PrivacyChoice::Contacts),
                (PrivacyKind::About, PrivacyChoice::Everyone),
                (PrivacyKind::GroupAdd, PrivacyChoice::Contacts),
                (PrivacyKind::ReadReceipts, PrivacyChoice::Everyone),
                (PrivacyKind::CallAdd, PrivacyChoice::Everyone),
            ],
            false,
        );
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_write_restores_the_confirmed_value() {
        let mut snapshot = Snapshot::demo();
        assert!(snapshot.begin_set(PrivacyKind::Profile, PrivacyChoice::Nobody));
        assert_eq!(
            snapshot.get(PrivacyKind::Profile),
            Some(PrivacyChoice::Nobody)
        );
        assert!(
            !snapshot.begin_set(PrivacyKind::Profile, PrivacyChoice::Everyone),
            "a second write waits for the first"
        );
        snapshot.fail_set(PrivacyKind::Profile);
        assert_eq!(
            snapshot.get(PrivacyKind::Profile),
            Some(PrivacyChoice::Contacts)
        );
        assert!(!snapshot.pending(PrivacyKind::Profile));
    }

    #[test]
    fn a_fetch_during_a_write_keeps_it_pending() {
        let mut snapshot = Snapshot::demo();
        assert!(snapshot.begin_set(PrivacyKind::Profile, PrivacyChoice::Nobody));
        snapshot.apply_fetch(
            vec![
                (PrivacyKind::Profile, PrivacyChoice::Everyone),
                (PrivacyKind::About, PrivacyChoice::Nobody),
            ],
            false,
        );
        assert!(snapshot.pending(PrivacyKind::Profile));
        assert_eq!(
            snapshot.get(PrivacyKind::Profile),
            Some(PrivacyChoice::Nobody)
        );
        assert_eq!(
            snapshot.get(PrivacyKind::About),
            Some(PrivacyChoice::Nobody)
        );
        // A refusal now restores what the fetch reported.
        snapshot.fail_set(PrivacyKind::Profile);
        assert_eq!(
            snapshot.get(PrivacyKind::Profile),
            Some(PrivacyChoice::Everyone)
        );
    }

    #[test]
    fn a_failed_fetch_keeps_the_values_but_stops_editing() {
        let mut snapshot = Snapshot::demo();
        assert!(snapshot.editable());
        snapshot.apply_fetch(Vec::new(), true);
        assert!(!snapshot.editable());
        assert_eq!(
            snapshot.get(PrivacyKind::Profile),
            Some(PrivacyChoice::Contacts)
        );
        snapshot.apply_fetch(vec![(PrivacyKind::Profile, PrivacyChoice::Nobody)], false);
        assert!(snapshot.editable());
        assert_eq!(
            snapshot.get(PrivacyKind::LastSeen),
            None,
            "the fetch is whole"
        );
    }

    #[test]
    fn nothing_is_written_for_an_unknown_or_unchanged_category() {
        let mut snapshot = Snapshot::default();
        assert!(!snapshot.begin_set(PrivacyKind::Profile, PrivacyChoice::Nobody));
        let mut snapshot = Snapshot::demo();
        assert!(!snapshot.begin_set(PrivacyKind::Profile, PrivacyChoice::Contacts));
        assert!(
            !snapshot.finish_set(PrivacyKind::Profile),
            "nothing was waiting"
        );
    }
}
