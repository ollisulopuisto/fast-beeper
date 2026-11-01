# Changelog

Newest first. One `## <version>` section per release, bullets under it;
the app shows the sections newer than the version the user last
acknowledged, and the latest sections when opened from Help. The
`## Unreleased` section collects notes for the next release and is never
shown automatically; move its bullets under the release version when the
version in Cargo.toml is bumped.

## Unreleased

## v0.21.0

- Command palette (Ctrl+P): find chats, apply list filters and run common actions from one keyboard-first overlay.
- Combined sidebar: network and account filters moved into the filter chip row, pinned chats show as regular rows at the top of the list, and Calls stays reachable in the rail with Beeper accounts.
- The chat list's edge can be dragged to resize it.
- Attachment uploads to WhatsApp no longer send the request shape that was rejected; a failed send keeps the staged files.
- The sidebar restores correctly in narrow windows and with the list hidden, and settings rows no longer cut their titles short.
