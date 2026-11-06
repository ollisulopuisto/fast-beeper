//! Favorite chats, kept in the phone's order and synced with it.
//!
//! WhatsApp stores the favorites as one app-state value holding the whole
//! ordered list, so the phone's latest list replaces ours. A change made here
//! is applied at once and also queued in `favorite_changes` until the phone
//! has our list: a phone list that arrives first is taken as it is and the
//! queued changes are replayed on top, so neither side loses an edit.
//!
//! `favorites_synced_at` in `meta` is the time of the newest list applied,
//! received or sent. Until it exists the phone's list is unknown and nothing
//! may be sent, because the whole list would overwrite the phone's.

use std::collections::HashSet;

use rusqlite::Connection;

use super::{Archive, OptionalExtension, Result, params};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS favorites (
    chat TEXT PRIMARY KEY,
    jid TEXT NOT NULL,
    position INTEGER NOT NULL,
    source TEXT NOT NULL DEFAULT 'manual'
);
CREATE TABLE IF NOT EXISTS favorite_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    chat TEXT NOT NULL,
    favorite INTEGER NOT NULL
);
";

const SYNCED_AT: &str = "favorites_synced_at";

/// One favorite as the phone names it: the canonical chat id for the list
/// here, and the JID to send back so the phone recognises its own entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Favorite {
    pub chat: String,
    pub jid: String,
}

/// Favorites marked by a build that kept them on this computer only lived in
/// `chats.favorite`. Each becomes a queued addition, merged into the phone's
/// list once it is known.
pub(super) fn adopt_local_marks(connection: &Connection) -> Result<()> {
    let legacy = connection
        .prepare("PRAGMA table_info(chats)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.as_deref() == Ok("favorite"));
    if !legacy {
        return Ok(());
    }
    let transaction = connection.unchecked_transaction()?;
    connection.execute_batch(
        "INSERT OR IGNORE INTO favorites (chat, jid, position)
             SELECT id, id, (SELECT COALESCE(MAX(position), -1) FROM favorites) + ROW_NUMBER() OVER (ORDER BY rowid)
             FROM chats WHERE favorite = 1 AND id NOT LIKE '%@newsletter';
         INSERT INTO favorite_changes (chat, favorite)
             SELECT id, 1 FROM chats WHERE favorite = 1 AND id NOT LIKE '%@newsletter' ORDER BY rowid;
         UPDATE chats SET favorite = 0 WHERE favorite = 1;",
    )?;
    transaction.commit()
}

impl Archive {
    /// The favorites in order.
    pub fn favorites(&self) -> Result<Vec<Favorite>> {
        let mut statement = self
            .connection
            .prepare("SELECT chat, jid FROM favorites ORDER BY position, chat")?;
        let rows = statement.query_map([], |row| {
            Ok(Favorite {
                chat: row.get(0)?,
                jid: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    /// When the newest favorites list applied here was made, or `None` while
    /// the phone's list is still unknown.
    pub fn favorites_synced_at(&self) -> Result<Option<i64>> {
        Ok(self.meta(SYNCED_AT)?.and_then(|value| value.parse().ok()))
    }

    /// Adds a favorite at the end of the list, or removes one, and queues the
    /// change for the phone. Returns whether anything changed.
    pub fn set_favorite(&self, chat: &str, favorite: bool) -> Result<bool> {
        let transaction = self.connection.unchecked_transaction()?;
        let changed = if favorite {
            // A manual mark on a pin-derived row makes it stick: a later
            // unpin in Beeper no longer removes it.
            self.connection.execute(
                "UPDATE favorites SET source = 'manual' WHERE chat = ?1 AND source = 'pin'",
                params![chat],
            )?;
            self.connection.execute(
                "INSERT OR IGNORE INTO favorites (chat, jid, position)
                 VALUES (?1, ?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM favorites))",
                params![chat],
            )?
        } else {
            self.connection
                .execute("DELETE FROM favorites WHERE chat = ?1", params![chat])?
        } > 0;
        if changed {
            self.connection.execute(
                "INSERT INTO favorite_changes (chat, favorite) VALUES (?1, ?2)",
                params![chat, favorite],
            )?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    /// Moves a favorite to `target`, its new zero-based place, and renumbers
    /// the list densely so the bar and Command+1..9 never see a gap. Returns
    /// the list after the move, or `None` when the chat is not a favorite.
    pub fn reorder_favorite(&self, chat: &str, target: usize) -> Result<Option<Vec<Favorite>>> {
        let mut list = self.favorites()?;
        let Some(from) = list.iter().position(|favorite| favorite.chat == chat) else {
            return Ok(None);
        };
        let entry = list.remove(from);
        list.insert(target.min(list.len()), entry);
        let transaction = self.connection.unchecked_transaction()?;
        for (position, favorite) in list.iter().enumerate() {
            self.connection.execute(
                "UPDATE favorites SET position = ?2 WHERE chat = ?1",
                params![favorite.chat, position as i64],
            )?;
        }
        transaction.commit()?;
        Ok(Some(list))
    }

    /// Mirrors a Beeper pin into a favorite, or removes the mirror when the
    /// pin is gone. Only pin-derived rows are touched: a favorite the user
    /// marked here survives an unpin, and the phone's queue never hears
    /// about mirrors. Returns whether the list changed.
    pub fn mirror_pin(&self, chat: &str, pinned: bool) -> Result<bool> {
        let changed = if pinned {
            self.connection.execute(
                "INSERT OR IGNORE INTO favorites (chat, jid, position, source)
                 VALUES (?1, ?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM favorites), 'pin')",
                params![chat],
            )?
        } else {
            self.connection.execute(
                "DELETE FROM favorites WHERE chat = ?1 AND source = 'pin'",
                params![chat],
            )?
        } > 0;
        Ok(changed)
    }

    /// Whether the chat is a pin-derived favorite, so the interface can
    /// unpin instead of unfavoriting.
    pub fn is_pin_derived(&self, chat: &str) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM favorites WHERE chat = ?1 AND source = 'pin'",
            params![chat],
            |row| row.get::<_, i64>(0),
        )? > 0)
    }

    /// The newest queued change, when any wait for the phone.
    pub fn pending_favorites(&self) -> Result<Option<i64>> {
        self.connection
            .query_row("SELECT MAX(seq) FROM favorite_changes", [], |row| {
                row.get(0)
            })
            .optional()
            .map(Option::flatten)
    }

    /// Replaces the list with the phone's, made at `at` (milliseconds), then
    /// replays the changes still queued here. An older list than the newest
    /// applied is ignored and returns `None`; otherwise returns every chat
    /// whose mark or place may have moved.
    pub fn apply_phone_favorites(&self, list: &[Favorite], at: i64) -> Result<Option<Vec<String>>> {
        if self
            .favorites_synced_at()?
            .is_some_and(|synced| at < synced)
        {
            return Ok(None);
        }
        let transaction = self.connection.unchecked_transaction()?;
        let before = self.favorites()?;
        self.connection.execute("DELETE FROM favorites", [])?;
        let mut seen = HashSet::new();
        let mut position = 0_i64;
        for favorite in list {
            if seen.insert(favorite.chat.as_str()) {
                self.connection.execute(
                    "INSERT INTO favorites (chat, jid, position) VALUES (?1, ?2, ?3)",
                    params![favorite.chat, favorite.jid, position],
                )?;
                position += 1;
            }
        }
        let queued: Vec<(String, bool)> = {
            let mut statement = self
                .connection
                .prepare("SELECT chat, favorite FROM favorite_changes ORDER BY seq")?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_>>()?
        };
        for (chat, favorite) in queued {
            if favorite {
                self.connection.execute(
                    "INSERT OR IGNORE INTO favorites (chat, jid, position)
                     VALUES (?1, ?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM favorites))",
                    params![chat],
                )?;
            } else {
                self.connection
                    .execute("DELETE FROM favorites WHERE chat = ?1", params![chat])?;
            }
        }
        self.set_meta(SYNCED_AT, &at.to_string())?;
        let after = self.favorites()?;
        transaction.commit()?;
        let mut touched: Vec<String> = Vec::new();
        for favorite in before.into_iter().chain(after) {
            if !touched.contains(&favorite.chat) {
                touched.push(favorite.chat);
            }
        }
        Ok(Some(touched))
    }

    /// The phone accepted our list, made at `at`, holding every change up to
    /// `through`. Later changes stay queued.
    pub fn favorites_sent(&self, through: i64, at: i64) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        self.connection.execute(
            "DELETE FROM favorite_changes WHERE seq <= ?1",
            params![through],
        )?;
        let at = self
            .favorites_synced_at()?
            .map_or(at, |synced| synced.max(at));
        self.set_meta(SYNCED_AT, &at.to_string())?;
        transaction.commit()
    }

    /// Files a favorite made under a privacy id under the phone number.
    /// Returns whether one moved.
    pub(super) fn move_favorite(
        transaction: &rusqlite::Transaction<'_>,
        from: &str,
        to: &str,
    ) -> Result<bool> {
        let moved = transaction.execute(
            "UPDATE OR IGNORE favorites SET chat = ?2 WHERE chat = ?1",
            params![from, to],
        )? > 0;
        transaction.execute("DELETE FROM favorites WHERE chat = ?1", params![from])?;
        transaction.execute(
            "UPDATE favorite_changes SET chat = ?2 WHERE chat = ?1",
            params![from, to],
        )?;
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn favorite(chat: &str) -> Favorite {
        Favorite {
            chat: chat.into(),
            jid: chat.into(),
        }
    }

    fn chats(archive: &Archive) -> Vec<String> {
        archive
            .favorites()
            .unwrap()
            .into_iter()
            .map(|favorite| favorite.chat)
            .collect()
    }

    #[test]
    fn the_phone_list_replaces_ours_in_its_order() {
        let archive = Archive::in_memory().unwrap();
        archive
            .apply_phone_favorites(&[favorite("b"), favorite("a")], 10)
            .unwrap();
        archive.favorites_sent(i64::MAX, 10).unwrap();
        let touched = archive
            .apply_phone_favorites(&[favorite("c"), favorite("b"), favorite("c")], 20)
            .unwrap()
            .expect("newer");
        assert_eq!(chats(&archive), ["c", "b"]);
        assert!(touched.contains(&"a".to_owned()), "a lost its mark");
        assert_eq!(archive.favorites_synced_at().unwrap(), Some(20));
    }

    #[test]
    fn an_older_list_is_ignored() {
        let archive = Archive::in_memory().unwrap();
        archive.apply_phone_favorites(&[favorite("a")], 20).unwrap();
        assert!(
            archive
                .apply_phone_favorites(&[favorite("b")], 10)
                .unwrap()
                .is_none()
        );
        assert_eq!(chats(&archive), ["a"]);
    }

    #[test]
    fn changes_made_before_the_phone_list_replay_on_top_of_it() {
        let archive = Archive::in_memory().unwrap();
        assert!(archive.set_favorite("mine", true).unwrap());
        assert!(archive.set_favorite("gone", true).unwrap());
        assert!(archive.set_favorite("gone", false).unwrap());
        assert!(!archive.set_favorite("gone", false).unwrap(), "no change");
        assert_eq!(archive.favorites_synced_at().unwrap(), None);
        archive
            .apply_phone_favorites(&[favorite("phone"), favorite("gone")], 5)
            .unwrap();
        assert_eq!(chats(&archive), ["phone", "mine"]);
        let through = archive.pending_favorites().unwrap().expect("queued");
        archive.set_favorite("later", true).unwrap();
        archive.favorites_sent(through, 7).unwrap();
        assert!(
            archive.pending_favorites().unwrap().is_some(),
            "later waits"
        );
        assert_eq!(archive.favorites_synced_at().unwrap(), Some(7));
    }

    #[test]
    fn marks_from_a_local_only_build_join_the_queue_once() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE chats (id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL,
                    favorite INTEGER NOT NULL DEFAULT 0);
                 CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO chats VALUES ('x@s.whatsapp.net', 'X', 'direct', 1);
                 INSERT INTO chats VALUES ('y@s.whatsapp.net', 'Y', 'direct', 0);
                 INSERT INTO chats VALUES ('n@newsletter', 'N', 'channel', 1);",
            )
            .unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        adopt_local_marks(&connection).unwrap();
        adopt_local_marks(&connection).unwrap();
        let archive = Archive { connection };
        assert_eq!(chats(&archive), ["x@s.whatsapp.net"]);
        archive
            .apply_phone_favorites(&[favorite("p@s.whatsapp.net")], 1)
            .unwrap();
        assert_eq!(chats(&archive), ["p@s.whatsapp.net", "x@s.whatsapp.net"]);
    }

    #[test]
    fn a_reorder_moves_and_renumbers_densely() {
        let archive = Archive::in_memory().unwrap();
        for chat in ["a", "b", "c", "d"] {
            archive.set_favorite(chat, true).unwrap();
        }
        let list = archive.reorder_favorite("d", 1).unwrap().expect("favorite");
        assert_eq!(
            list.iter()
                .map(|favorite| favorite.chat.as_str())
                .collect::<Vec<_>>(),
            ["a", "d", "b", "c"]
        );
        assert_eq!(chats(&archive), ["a", "d", "b", "c"]);
        // The stored numbering is dense after the move.
        let positions: Vec<i64> = archive
            .connection
            .prepare("SELECT position FROM favorites ORDER BY position")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(positions, [0, 1, 2, 3]);
    }

    #[test]
    fn a_reorder_past_the_end_lands_last_and_a_stranger_is_ignored() {
        let archive = Archive::in_memory().unwrap();
        for chat in ["a", "b", "c"] {
            archive.set_favorite(chat, true).unwrap();
        }
        let list = archive
            .reorder_favorite("a", 99)
            .unwrap()
            .expect("favorite");
        assert_eq!(
            list.iter()
                .map(|favorite| favorite.chat.as_str())
                .collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
        assert!(archive.reorder_favorite("stranger", 0).unwrap().is_none());
        assert_eq!(chats(&archive), ["b", "c", "a"]);
    }

    #[test]
    fn a_pinned_chat_mirrors_into_a_favorite_and_back() {
        let archive = Archive::in_memory().unwrap();
        archive.mirror_pin("a", true).unwrap();
        assert!(archive.is_pin_derived("a").unwrap());
        assert_eq!(chats(&archive), ["a"]);
        // Mirroring the same pin again changes nothing.
        assert!(!archive.mirror_pin("a", true).unwrap());
        // The mirror never reaches the phone's queue.
        assert!(archive.pending_favorites().unwrap().is_none());
        archive.mirror_pin("a", false).unwrap();
        assert!(chats(&archive).is_empty());
        assert!(!archive.is_pin_derived("a").unwrap());
    }

    #[test]
    fn a_manual_mark_on_a_mirror_survives_the_unpin() {
        let archive = Archive::in_memory().unwrap();
        archive.mirror_pin("a", true).unwrap();
        archive.set_favorite("a", true).unwrap();
        assert!(!archive.is_pin_derived("a").unwrap());
        archive.mirror_pin("a", false).unwrap();
        assert_eq!(chats(&archive), ["a"], "the manual mark sticks");
    }

    #[test]
    fn a_mirror_joins_after_the_manual_favorites() {
        let archive = Archive::in_memory().unwrap();
        archive.set_favorite("manual", true).unwrap();
        archive.mirror_pin("pinned", true).unwrap();
        assert_eq!(chats(&archive), ["manual", "pinned"]);
    }

    #[test]
    fn a_reorder_survives_a_gapped_numbering() {
        let archive = Archive::in_memory().unwrap();
        for chat in ["a", "b", "c"] {
            archive.set_favorite(chat, true).unwrap();
        }
        archive.set_favorite("b", false).unwrap();
        let list = archive.reorder_favorite("c", 0).unwrap().expect("favorite");
        assert_eq!(
            list.iter()
                .map(|favorite| favorite.chat.as_str())
                .collect::<Vec<_>>(),
            ["c", "a"]
        );
    }
}
