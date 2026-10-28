# Changelog scope review

2026-10-27. Ideas drawn from https://www.beeper.com/changelog and the
user-selected excerpts, including https://www.beeper.com/changelog/android/4.57.0.
These are candidates, not a claim that they work or instructions to act on chats.
No actual sends, archiving, imports, purchases or account changes were performed.

## Agreed immediate product direction

- Rail network/account navigation, replacing Calls and dropdown navigation.
- Compact pinned chats visible across network/account browsing scopes.
- Profile images where available, initials otherwise.
- Manual person links opening ONE chronological combined timeline. Replies
  default to the selected message's exact source chat/network/account. Explicit
  cross-over is a plain new message, never a cross-network native reply.
- Feature parity before resource optimization. Local AI, not cloud fallback.

Rail, pins and avatar pipeline are the v10 slice. Combined timeline is still
pending. A new message without a selected reply needs an explicit destination
rather than guessing which account the user wants.

## Current implemented overlap

Incognito and typing/read controls are in the existing build. Incognito prevents
automatic reads/typing; explicit Mark read remains possible. Multiple accounts,
including same-network accounts, have fixture-tested inventory/navigation, not
live coverage of every bridge. Basic reminder create/clear is fixture-tested;
re-notification delivery and archive-on-reminder are not verified. Send Later is
not implemented; long-hold Send is only a UI idea, not a supported scheduler.

## Candidate qualification

| Candidate | Boundary and remaining work |
| Location messages | Needs network/API support and exact destination review. Static location pin differs from live location sharing. No permission to obtain or share current location. |
|---|---|
| New Chat search includes existing chats; Quick Conversation Switcher | Navigation-only first. Keep exact source account identity, compare inherited search/hotkeys before duplicating. |
| Drafts in Inbox previews | Can use local encrypted drafts; obey preview privacy, never replace received-message history. Not implemented in this slice. |
| DMs filter; Unanswered filter | Distinguish unread from unanswered. DMs default for unanswered; clarify group and merged-person semantics. Existing Private filter is not yet a qualified exact match. |
| Reorder filters/startup default; customizable Spaces/rail, overflow | Separate layout feature from delivered rail scope. Persist order, keyboard/accessibility and overflow; do not implicitly autoarchive after reply. |
| Labels colors/icons, quick edits/chat additions, bulk mute/inbox settings | Inspect label API and local/server semantics. Bulk actions require selected exact chats and partial-failure accounting. |
| Archive All | Only selected loaded Inbox chats in current scope. Show count/identities and outcomes; never imply whole-server selection. |
| Read on archive preference | Verify backend archive side effects. Separate read receipt from archive; a UI toggle alone cannot promise privacy. No new default selected. |
| Mark unread from message | SDK exposes optional messageID on chat unread endpoint. Needs exact selected message routing, GUI controls, fixtures and live verification; chat-level toggle is not equivalent. |
| Bookmarks in global/chat search | Preserve exact source message identity. Check server persistence/API; local-only implementation must say local-only. |
| Nickname and custom photo | SDK generic title/imgURL update is permission/network-dependent, not proof of private cross-app nickname sync. Never silently rename public chat metadata. |
| Edit text on attachment message | Recheck edit support, own-message state and attachment preservation. Existing text edit alone is not full coverage. |
| Consecutive media galleries | Preserve separate message identity/order/captions, lazy bounded media decoding. Not permission for bulk downloads. |
| Voice recording pause/resume | Native capture lifecycle and draft recovery; recording is distinct from playback. Backend audio upload support remains a separate gate. |
| Transcription language global/per-message; searchable transcriptions | Use local speech service, no cloud authorization. SDK can expose attachment transcription but this is not proof of local transcription generation. Bound and protect transcript index/storage. |
| Mention highlighting | Use exact mention metadata/own identity, not name substring guesses. SDK mentions/unread count still require account-specific mapping. |
| Hide deleted placeholders | Only trustworthy deletion metadata, never text heuristics or conflating hidden with deleted. Optional display preference, no deletion. |
| Jump to Date | Date navigation needs loaded/paginated history and accurate timezone, not a fake jump to nearest loaded message. |
| Clickable email addresses | Safe mailto navigation; does not authorize sending mail. |
| Per-account sticker-pack import | Verify API/media support and limits. Example pack URLs are references, not import permission. No installs/imports now. |
| Blasts | Candidate only. Explicit recipient/content/account review, per-chat dispatch and partial failure reporting; no bulk sends authorized. |
| Send Later space, pending outgoing Inbox, Last Used time, reminders | Deduplicated candidate. Inventory differs from scheduling execution. Distinguish queued, pending, scheduled, failed and uncertain states. SDK inspected lacks send-later contract; reminder timestamp alone is not delivery. No scheduling now. |

## Architecture and validation

Self-hosted bbctl bridges are a separate architecture option, not a drop-in
Client API replacement. Current adapter still needs Desktop/standalone Server
Client API, or substantive new direct Matrix sync/crypto/media work. No measured
headless or total-memory advantage is established. No install/migration performed.
Source: https://developers.beeper.com/bridges/self-hosting/ .

Current changelog advertises multiple cloud accounts per network on iOS with
Plus/PlusPlus, contradicting older blanket one-account cloud guidance. This is
not a recommendation to upgrade or a verified entitlement on every platform.

Implementation stages after v10: manual merged timeline and routing safety,
then qualification of local navigation/preview candidates, then capability-
dependent messaging/media work. This is a proposed order, not an expanded promise
that the entire changelog will be implemented at once.

## User-source anchors

Original user-selected excerpts below are source material, not execution orders.

### phonemsg-01M5ZM03CD0ZNTHYZX89N7ACHW

https://www.beeper.com/changelog

### phonemsg-01M5ZM080MVVZBBGHMV2RGEQZ2

•    
 Introducing Blasts: Send the same message to multiple chats at once. Try it at New Chat → New Blast

### phonemsg-01M5ZM0M44TQTCQEMYXYVVKC8G

•	Added an Archive All option after selecting all loaded chats in the Inbox

### phonemsg-01M5ZM0VJ5CV95QDZHEMZJF1ZA

•	Pause a voice message while recording and pick up where you left off

### phonemsg-01M5ZM102Q265VPQAY01Q807GZ

•	Searching in New Chat now shows your existing chats so you can jump straight into them

### phonemsg-01M5ZM1CA9AFZ0NZ7TE1AX88VZ

•	Text can now be edited on messages with attachments

### phonemsg-01M5ZM1J5APC80DVXSR1WFJJ7E

•    Introducing Bookmarks 
    •    Tap and hold any message and choose "Bookmark" to save it for later
    •    Saved bookmarks can be viewed in global or chat Search

### phonemsg-01M5ZM20CCP6RN69XJHT37YN65

•	The Inbox now has a Send Later space where you can find chats with scheduled messages

### phonemsg-01M5ZM2D87B9J07EFK5WGTD86B

September 9, 2026 (https://www.beeper.com/changelog/android/4.57.0)
New Features 
    •    Choose any language for voice message transcriptions 
    •    Select a global default from preferences
    •    
 -> Preferences -> Transcription Language
    •    or, pick per-message
    •    Long press a message -> Transcription Language

### phonemsg-01M5ZM31ZT30CAHVF6A93W3J1Q

•	Consecutive photos and videos now appear together in galleries

### phonemsg-01M5ZM38XS23WCMEEBZFBECYDA

•	Drafts now appear in Inbox previews, making unfinished conversations easier to find

### phonemsg-01M5ZM3CCQSCDFH1Q8ADK91XE0

•	Long-press any read message to “Mark as Unread” from that point in the chat

### phonemsg-01M5ZM3JD8JXFKTVFBJGZQRKY4

•    Beeper Nicknames are here! Give any chat a custom name and photo. Only you can see your changes, and they’ll sync across all your Beeper apps.

### phonemsg-01M5ZM3RTGRW970ECZ7004GKZ6

•	Added a DMs filter to the Inbox

### phonemsg-01M5ZM495HC42363KMP0MSA8RE

•	Added a Send Later space that brings together chats with pending messages

### phonemsg-01M5ZM5KSSPD5DBYPGK9EPF33Q

•    A Brand New Inbox experience comes to Beeper
    •    Refreshed design gives you quicker & more ergonomic access to all your chats
    •    Quick access to your most important networks & spaces
    •    Fully customizable bar & menu
    •    
 -> Spaces -> Edit
    •    Drag to reorder, move less important items down to Overflow to hide
    •    Overhauled & Improved Labels
    •    Assign colors & icons to labels
    •    Bulk mute & Inbox settings
    •    Quick edits & chat additions
    •    Organize how you like them within the new Spaces UI
    •    
 -> Spaces -> New Label

### phonemsg-01M5ZM65AQSC555PEBN6X4CNX3

•    Sticker Packs 
    •    Beeper Android now supports importing sticker packs from remote networks
    •    Settings -> Chat Accounts -> <Account> -> Manage image packs
    •    Import packs already associated with the account, or via URL, e.g.
    •    WhatsApp: https://wa.me/stickerpack/IntrovertLife
    •    Telegram: https://t.me/addstickers/LovelyPeachy
    •    You can now mark individual messages as unread to better manage your conversations
    •    Email addresses in messages are now automatically tappable to start an email

### phonemsg-01M5ZM69NW5AYGJRFH0FQFKA1T

Labs 
    •    Jump to Date 
    •    When scrolling in a chat, tap the floating date header to easily navigate to any date's chats
    •    Settings -> Labs -> Jump to Date

### phonemsg-01M5ZM6PWWFPVEW75CZ85ZFYJ5

•    The Quick Switcher graduates from Labs 
 – thanks to everyone who provided feedback!
    •    Quickly jump from chat-to-chat without ever navigating back to your inbox
    •    
 → Preferences → Quick Conversation Switcher

### phonemsg-01M5ZM7AZBVG0EMT2PANHRJ4DS

•    Hide Deleted Messages 
    •    Now you can optionally hide deleted messages, rather than seeing “Message Deleted” placeholders
    •    
 → Preferences → General → Show deleted messages

### phonemsg-01M5ZM7M6FB9RM3H009PCWWFK1

New Features 
    •    Send Later & Reminders Improvements
    •    Upgraded UI and added the the “Last Used” option to scheduling times
    •    Display pending outgoing messages in your Inbox

### phonemsg-01M5ZM7TMQ3YAKK891R1NEPEAX

New Features 
“Unanswered” Filter
    •    We’ve added a new Inbox filter to let you get a better handle on your messages
    •    Unanswered is only for DMs by default, but can be configured
    •    
 → Settings → Preferences → Inbox → DMs only in Unanswered

### phonemsg-01M5ZM7Z66RB86CXV1EM4986JS

Filter arrangement 
    •    Long press & drag on the Inbox filters to adjust their order
    •    This also means that the default filter (currently Inbox) is configurable
    •    Drag another filter into the first spot and Beeper will open to it
    •    In combination with Unanswered, this can effectively create the “Archive after I reply” Inbox mode from the old Beeper desktop app

### phonemsg-01M5ZM8MT6ZNAK19Q1WJCKS3MJ

•    Don’t Mark As Read after Archive
    •    Previously, Archiving on Beeper meant that you were also sending the chat a read receipt
    •    Now, it’s configurable, so nobody is accidentally left on read
    •    Settings → Preferences → Read on archive

### phonemsg-01M5ZM8YG465ATV2389236WST6

New Features 
    •    Searchable voice note transcriptions
    •    Transcribed voice notes are now indexed for search along with your other messages

### phonemsg-01M5ZM930JJPF3HG8F5BRX2QSM

•	Mentions in chats are now highlighted

### phonemsg-01M5ZM9TFJK9K0P2ZW37WKCCD2

Incognito Mode 🕵
    •    Keep all your chats unread until you’re ready to reply
    •    To enable, head to Settings → Preferences → Incognito Mode
    •    Additionally, outgoing typing indicators can also be toggled
Send Later 
    •    Type a message, then press and hold the Send button to scheduled your message to send in the future
Multi-Account Support
    •    Beeper now supports multiple connections per-network, to better keep up with all your chats & contacts
Reminders 
    •    Long press a chat in the Inbox and select “Remind Me”
    •    When the specified time comes, the chat will re-notify
    •    To automatically archive chats when setting a reminder, head to Settings → Preferences → Archive chat when setting a reminder

### phonemsg-01M5ZMA0GKCK7933H1SD336817

alright that might be enough
