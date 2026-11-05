//! A call's entry among a chat's messages: the bubble's glyph, title and
//! detail line, read from the message itself.

use crate::i18n::{Locale, gettext};
use crate::model::{CallStatus, Content, Message};
use crate::theme::Icon;

/// One call, as its entry reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Whether this account placed the call.
    pub from_me: bool,
    pub video: bool,
    pub status: CallStatus,
    /// Seconds the call was up.
    pub duration: u64,
    pub group: bool,
}

impl Entry {
    /// The entry of a call message, if it is one.
    pub fn of(message: &Message) -> Option<Self> {
        match message.content {
            Content::Call {
                video,
                status,
                duration,
                group,
            } => Some(Self {
                from_me: message.from_me,
                video,
                status,
                duration,
                group,
            }),
            _ => None,
        }
    }

    /// One glyph for the state: the arrow says which way an answered call
    /// went, the camera what it carried, and a call that never connected has
    /// the missed or the slashed handset.
    pub fn icon(&self) -> Icon {
        match self.status {
            CallStatus::Missed | CallStatus::NoAnswer => Icon::PhoneMissed,
            CallStatus::Declined
            | CallStatus::DeclinedElsewhere
            | CallStatus::Busy
            | CallStatus::Failed
            | CallStatus::ConnectionLost => Icon::PhoneOff,
            CallStatus::Answered | CallStatus::AnsweredElsewhere => {
                if self.video {
                    Icon::Video
                } else if self.from_me {
                    Icon::PhoneOutgoing
                } else {
                    Icon::PhoneIncoming
                }
            }
        }
    }

    /// Whether this was a call to us that nobody took.
    pub fn missed(&self) -> bool {
        self.status == CallStatus::Missed && !self.from_me
    }

    /// What the call was: "Missed voice call", "Video call", "Group voice call".
    pub fn title(&self, locale: Locale) -> String {
        let text = match (self.missed(), self.group, self.video) {
            (true, false, false) => gettext(locale, "Missed voice call"),
            (true, false, true) => gettext(locale, "Missed video call"),
            (true, true, false) => gettext(locale, "Missed group voice call"),
            (true, true, true) => gettext(locale, "Missed group video call"),
            (false, false, false) => gettext(locale, "Voice call"),
            (false, false, true) => gettext(locale, "Video call"),
            (false, true, false) => gettext(locale, "Group voice call"),
            (false, true, true) => gettext(locale, "Group video call"),
        };
        text.into_owned()
    }

    /// How it went: its length when it was up, else what became of it. A
    /// missed call says it in its title.
    pub fn detail(&self, locale: Locale) -> Option<String> {
        let text = match self.status {
            CallStatus::Answered if self.duration > 0 => {
                return Some(crate::util::call_length(locale, self.duration));
            }
            CallStatus::Answered | CallStatus::Missed => return None,
            CallStatus::AnsweredElsewhere => gettext(locale, "Answered on another device"),
            CallStatus::Declined => gettext(locale, "Declined"),
            CallStatus::DeclinedElsewhere => gettext(locale, "Declined on another device"),
            CallStatus::Busy => gettext(locale, "Busy"),
            CallStatus::Failed => gettext(locale, "Could not connect"),
            CallStatus::NoAnswer => gettext(locale, "No answer"),
            CallStatus::ConnectionLost => gettext(locale, "Connection lost"),
        };
        Some(text.into_owned())
    }
}
