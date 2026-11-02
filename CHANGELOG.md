# Changelog

Newest first. One `## <version>` section per release, bullets under it;
the app shows the sections newer than the version the user last
acknowledged, and the latest sections when opened from Help. The
`## Unreleased` section collects notes for the next release and is never
shown automatically; move its bullets under the release version when the
version in Cargo.toml is bumped.

## Unreleased

## v0.22.0

- Link a person's chats from Contact info: a searchable picker suggests name matches across networks, and the linked conversation shows who sent each message and on which network.
- The left rail is a single compact icon column: favorites and pinned chats are avatar chips with unread badges, filters are one icon row, and networks sit as plates at the bottom with right-click account scoping.
- Voice notes can be transcribed entirely on your own local speech services (no cloud), with a per-message transcription language override from the message menu.
- The conversation background picks up a light tint of the open chat's network color, and each Beeper account can have its own background and conversation theme, linked view included.
- Pinned chats keep their normal row in the chat list as well as their rail shortcut.
- Shortcuts: Command+1..9 opens the nth favorite, Option+1..n picks a filter.
- Quote cards show the original message's excerpt when it is already loaded, and Signal group mentions render as proper chips with real member names.
- Hide and mute indefinitely now holds locally and tells you when the bridge does not confirm the change.
- Send Later: adding a message no longer kills the scheduler, setup failures name the failing stage, and a resend that could duplicate a message asks for explicit confirmation.
- Clicking the Dock icon brings the window forward again on the packaged app.
- Connection warnings in a chat collapse to one line per distinct problem, with the full text on hover.
- Networking update: connections no longer hang on networks with broken IPv6, plus a TLS verification fix (ureq 3.5).

## v0.21.0

- Command palette (Ctrl+P): find chats, apply list filters and run common actions from one keyboard-first overlay.
- Combined sidebar: network and account filters moved into the filter chip row, pinned chats show as regular rows at the top of the list, and Calls stays reachable in the rail with Beeper accounts.
- The chat list's edge can be dragged to resize it.
- Attachment uploads to WhatsApp no longer send the request shape that was rejected; a failed send keeps the staged files.
- The sidebar restores correctly in narrow windows and with the list hidden, and settings rows no longer cut their titles short.
