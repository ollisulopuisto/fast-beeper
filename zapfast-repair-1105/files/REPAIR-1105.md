# Send, wrapping and keyboard repair

Against the delivered October28 08:55 source. Native Mac build/display of that
base succeeded on the user's Mac; this repair is locally fixture-tested only.

- Ordinary text sends omit optional replyToMessageID instead of sending null.
  Current official SDK specifies an optional string. Strict HTTP regression
  fails before/passes after. This is a likely400 cause, not a live send success.
- Error status now includes only known validation field paths, never arbitrary
  server text, token, input or private response values. No automatic resend.
- Filter chips wrap. Global pins wrap in a vertically scrollable area when needed;
  no inaccessible offscreen horizontal strip. Long message links wrap.
- Inline HTML in bodies and attachment captions becomes safe plain text, with
  line breaks and link label plus safe HTTP/HTTPS/mailto destination retained.
  Scripts/assets/CSS are not executed or loaded.
- Cmd1..9/0 networks, OptionCmd1..9/0 global pins, ControlCmd1..9/0 current
  non-pinned list on Mac, in visible order. Missing positions do nothing.
  On other platforms use Ctrl, AltCtrl and ShiftCtrl respectively.
- CmdShiftU toggles unread. CmdShiftR toggles Unreplied. This is conservative:
  a nonempty known sender and non-system/plain-text latest preview from others.
  Unknown/system/media-only preview cases are excluded, so this is not complete
  triage or a claim of reply obligation. The active chip is visible.
- The previous latest-loaded linked reply fallback moves to OptionCmdShiftR.
- Explicit persistent Reply later queue is approved next work, NOT in this repair.
- Live pagination warning from the user remains under investigation; the existing
  bounded retry/cursor protection remains. No claim that this repair fixes it.

165 portable Rust tests and26 Python tests pass; real GUI-library/binary build,
keyboard/wrap diagnostics and actual1100x780/700x700 synthetic windows inspected.
Full inherited GUI suite incomplete. No real-account test sends were made.

Quit ZapFast before applying. The updater verifies hashes against08:55 and refuses
any changed source, leaving user edits untouched. Build incrementally with Cargo;
do not rerun build_macos.py into the already-existing trial destination.
