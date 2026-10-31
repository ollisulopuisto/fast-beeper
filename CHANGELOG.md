# Changelog

Newest first. One `## <version>` section per release, bullets under it;
the app shows the sections newer than the version the user last
acknowledged, and the latest sections when opened from Help. The
`## Unreleased` section collects notes for the next release and is never
shown automatically; move its bullets under the release version when the
version in Cargo.toml is bumped.

## Unreleased

- Command palette (Ctrl+P): find chats, apply list filters and run common actions from one keyboard-first overlay.
- Combined sidebar: network and account filters moved into the filter chip row, pinned chats show as regular rows at the top of the list, and Calls stays reachable in the rail with Beeper accounts.
