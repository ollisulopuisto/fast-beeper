//! Local-only person links. Never send to a synthetic/automatic routing ID.
use crate::model::{Chat, Message};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Member {
    pub origin: String,
    pub account: String,
    pub chat: String,
}
impl Member {
    pub fn of(chat: &Chat) -> Option<Self> {
        Some(Self {
            origin: chat.backend_origin.clone()?,
            account: chat.backend_account.clone()?,
            chat: chat.id.clone(),
        })
    }
    pub fn matches(&self, chat: &Chat) -> bool {
        Self::of(chat).as_ref() == Some(self)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub members: Vec<Member>,
    #[serde(default)]
    pub primary: Option<Member>,
}
impl Person {
    pub fn default_destination<'a>(
        &'a self,
        available: &[Member],
    ) -> Result<Option<&'a Member>, &'static str> {
        match &self.primary {
            None => Ok(None),
            Some(primary) if self.members.contains(primary) && available.contains(primary) => {
                Ok(Some(primary))
            }
            Some(_) => Err(
                "Primary chat unavailable. Choose a destination explicitly; no fallback was selected.",
            ),
        }
    }
}
/// Exact same-member reply only. Cross-over is deliberately a new message.
pub fn reply_route(
    source: &Member,
    destination: &Member,
    id: &str,
) -> Option<crate::model::ReplyTarget> {
    (source == destination).then(|| crate::model::ReplyTarget {
        id: id.to_owned(),
        chat: None,
    })
}
/// Group only consecutive same-chat/sender/day runs, never different sources.
pub fn header_boundary(
    previous: Option<(&str, &str, &str, bool)>,
    current: (&str, &str, &str, bool),
) -> bool {
    previous != Some(current)
}
/// Overlapping rows and a small overscan are measured; distant rows use cached heights.
pub fn row_near_view(
    top: f32,
    height: f32,
    view_top: f32,
    view_bottom: f32,
    overscan: f32,
) -> bool {
    top + height >= view_top - overscan && top <= view_bottom + overscan
}
/// Literal case-insensitive search of already-loaded data. No remote query.
pub fn search_matches(body: &str, query: &str, network: &str, sender: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || [body, network, sender]
            .iter()
            .any(|value| value.to_lowercase().contains(&query))
}
/// Keyboard-focusable fallback operates on the same filtered, ordered rows as the view.
pub fn latest_replyable<'a>(messages: &[&'a Message]) -> Option<&'a Message> {
    messages
        .iter()
        .rev()
        .copied()
        .find(|message| !matches!(message.content, crate::model::Content::Revoked))
}
/// Sort pointer-sized references, not message text/media payloads, on every frame.
pub fn ordered_refs(mut messages: Vec<&Message>) -> Vec<&Message> {
    messages.sort_unstable_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then(a.chat.cmp(&b.chat))
            .then(a.history_order.cmp(&b.history_order))
            .then(a.id.cmp(&b.id))
    });
    messages.dedup_by(|a, b| a.chat == b.chat && a.id == b.id);
    messages
}
pub fn ordered(mut messages: Vec<Message>) -> Vec<Message> {
    messages.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then(a.chat.cmp(&b.chat))
            .then(a.history_order.cmp(&b.history_order))
            .then(a.id.cmp(&b.id))
    });
    messages.dedup_by(|a, b| a.chat == b.chat && a.id == b.id);
    messages
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_reply_uses_last_matching_exact_source_and_skips_revoked() {
        let mut chat = Chat::new("work".into(), "Work".into());
        chat.backend_origin = Some("server".into());
        chat.backend_account = Some("work-account".into());
        let first = Message {
            id: "same".into(),
            chat: "signal".into(),
            sender: "x".into(),
            sender_name: None,
            from_me: false,
            timestamp: 1,
            history_order: None,
            content: crate::model::Content::text("signal"),
            status: crate::model::Delivery::Sent,
            delivered_at: None,
            read_at: None,
            quoted: None,
            reactions: vec![],
            edited: false,
            mentions: vec![],
            forwarded: false,
            thumbnail: None,
        };
        let mut work = first.clone();
        work.chat = chat.id.clone();
        work.timestamp = 2;
        let mut revoked = work.clone();
        revoked.id = "revoked".into();
        revoked.timestamp = 3;
        revoked.content = crate::model::Content::Revoked;
        let rows = ordered_refs(vec![&revoked, &work, &first]);
        assert!(std::ptr::eq(latest_replyable(&rows).unwrap(), &work));
        assert_eq!(
            Member::of(&chat).unwrap().chat,
            latest_replyable(&rows).unwrap().chat
        );
        assert!(latest_replyable(&[]).is_none());
        assert!(latest_replyable(&[&revoked]).is_none());
    }
    #[test]
    fn consecutive_headers_never_hide_a_source_change() {
        assert!(header_boundary(None, ("signal", "friend", "one", false)));
        assert!(!header_boundary(
            Some(("signal", "friend", "one", false)),
            ("signal", "friend", "one", false)
        ));
        assert!(header_boundary(
            Some(("signal", "friend", "one", false)),
            ("work", "friend", "one", false)
        ));
        assert!(header_boundary(
            Some(("signal", "friend", "one", false)),
            ("signal", "friend", "two", false)
        ));
        assert!(header_boundary(
            Some(("signal", "friend", "one", false)),
            ("signal", "me", "one", true)
        ));
    }
    #[test]
    fn virtual_rows_include_edges_and_overscan_only() {
        assert!(row_near_view(0.0, 80.0, 80.0, 700.0, 0.0));
        assert!(row_near_view(800.0, 80.0, 80.0, 700.0, 120.0));
        assert!(!row_near_view(1000.0, 80.0, 80.0, 700.0, 120.0));
        assert!(!row_near_view(0.0, 80.0, 300.0, 700.0, 120.0));
    }
    #[test]
    fn loaded_search_is_literal_unicode_and_source_scoped() {
        assert!(search_matches(
            "Hyvää yötä, Alice",
            "Hyvää",
            "SIGNAL",
            "friend"
        ));
        assert!(search_matches("plain text", "signal", "Signal", "friend"));
        assert!(search_matches("plain text", "friend", "Work", "Friend"));
        assert!(!search_matches("plain text", "signal", "Work", "friend"));
        assert!(!search_matches("plain text", ".*", "Work", "friend"));
        assert!(search_matches("plain text", "  ", "Work", "friend"));
    }
    #[test]
    fn borrowed_timeline_preserves_sources_without_copying_payloads() {
        let make = |chat: &str, time| Message {
            id: "same".into(),
            chat: chat.into(),
            sender: "x".into(),
            sender_name: None,
            from_me: false,
            timestamp: time,
            history_order: None,
            content: crate::model::Content::text("x".repeat(1024 * 1024)),
            status: crate::model::Delivery::Sent,
            delivered_at: None,
            read_at: None,
            quoted: None,
            reactions: vec![],
            edited: false,
            mentions: vec![],
            forwarded: false,
            thumbnail: None,
        };
        let a = make("work", 2);
        let b = make("signal", 1);
        let refs = ordered_refs(vec![&a, &b]);
        assert!(std::ptr::eq(refs[0], &b));
        assert!(std::ptr::eq(refs[1], &a));
    }
    #[test]
    fn same_id_in_distinct_chats_is_not_deduplicated() {
        fn msg(chat: &str, time: i64) -> Message {
            Message {
                id: "collision".into(),
                chat: chat.into(),
                sender: "x".into(),
                sender_name: None,
                from_me: false,
                timestamp: time,
                history_order: None,
                content: crate::model::Content::text(chat),
                status: crate::model::Delivery::Sent,
                delivered_at: None,
                read_at: None,
                quoted: None,
                reactions: vec![],
                edited: false,
                mentions: vec![],
                forwarded: false,
                thumbnail: None,
            }
        }
        let rows = ordered(vec![msg("b", 2), msg("a", 1), msg("b", 2)]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].chat, "a");
        assert_eq!(rows[1].chat, "b");
    }
    #[test]
    fn primary_never_falls_back() {
        let first = Member {
            origin: "server".into(),
            account: "work".into(),
            chat: "one".into(),
        };
        let second = Member {
            origin: "server".into(),
            account: "personal".into(),
            chat: "two".into(),
        };
        let mut person = Person {
            name: "A".into(),
            members: vec![first.clone(), second.clone()],
            primary: None,
        };
        assert_eq!(
            person
                .default_destination(&[first.clone(), second.clone()])
                .unwrap(),
            None
        );
        person.primary = Some(first.clone());
        assert_eq!(
            person.default_destination(&[first.clone()]).unwrap(),
            Some(&first)
        );
        assert!(person.default_destination(&[second]).is_err());
    }
    #[test]
    fn crossover_never_quotes() {
        let a = Member {
            origin: "server".into(),
            account: "work".into(),
            chat: "one".into(),
        };
        let mut b = a.clone();
        b.account = "personal".into();
        assert!(reply_route(&a, &b, "same-id").is_none());
        b = a.clone();
        b.chat = "two".into();
        assert!(reply_route(&a, &b, "same-id").is_none());
        assert_eq!(reply_route(&a, &a, "same-id").unwrap().id, "same-id");
    }
    #[test]
    fn exact_origin_and_account_are_required() {
        let mut chat = Chat::new("same".into(), "A".into());
        chat.backend_origin = Some("one".into());
        chat.backend_account = Some("work".into());
        let member = Member::of(&chat).unwrap();
        chat.backend_origin = Some("two".into());
        assert!(!member.matches(&chat));
        chat.backend_origin = Some("one".into());
        chat.backend_account = Some("personal".into());
        assert!(!member.matches(&chat));
    }
}
