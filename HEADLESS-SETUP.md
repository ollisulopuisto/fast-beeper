# Standalone Server trial, separate from the working Desktop

This is a proposed, separately approved Server trial, not permission to perform
setup or reconnect accounts. A fresh Server session is an upstream-supported path,
but this prototype has not validated the user's networks or total memory savings.
Keep Desktop's existing connections/data untouched. Do not copy its databases,
remove accounts, log it out, enable remote access, or enable automatic startup
without separate approval.

These are local user-run steps, not actions already performed. Check commands
with the installed CLI's help if its version differs. No secrets in chat.

## 1. Inspect, then create a separate target

If the official CLI is not installed, its documented Homebrew installation is:

    brew install beeper/tap/cli

List existing targets first, without dumping private auth files:

    beeper targets list

If a dedicated trial target already exists, reuse it. Otherwise create a unique
name (example below), without --default. Stop if that name is already in use:

    beeper targets add server zapfast-headless

It uses a separate data directory/profile and selects a local port. Do not
assume Desktop's23373 or another guessed port. Every later command below names
this target explicitly. Existing Desktop target stays unchanged.

## 2. Sign in and verify locally

Use the actual email for the user's existing Beeper account, not an assumed
email from another service. Replace YOUR_BEEPER_EMAIL locally:

    beeper setup --server --install --target zapfast-headless --email YOUR_BEEPER_EMAIL

This installs/starts the separate Server, requests an emailed code, and guides
device verification. Enter code locally. Approve verification from the existing
signed-in Desktop. Never send tokens, cookies, passwords or recovery keys in
chat. No new Beeper account or paid entitlement is intended; stop if the flow
instead asks to create an account, accept new commercial terms or pay.

Inspect accounts already available to this Server:

    beeper --target zapfast-headless --read-only accounts list

Cloud connections may already appear. On-device connections require setup in
this new session. To add one via the supported guided chooser:

    beeper --target zapfast-headless accounts add

Choose the intended existing network account in that local flow. Start with
one network. Network-specific device limits/history availability still apply.
Do not recreate the entire mix until the first network survives restart.

## 3. Point ZapFast at exactly that target, read-only

From the new source package's client directory:

    uv run contrib/beeper-tools/run_client.py --beeper-target zapfast-headless

Or reuse an existing built binary with --binary and its actual path. The
launcher reads only that existing target's baseURL/token via the official CLI,
keeps credentials local and sets read-only. It doesn't start a stopped Server.
If CLI inspection supplies no token, the launcher asks locally rather than
using a Desktop token. Do not paste targets show/config JSON into chat.

Verify one chat opens and history is present. Quit Desktop normally (don't
log out or delete anything), then check ZapFast can still refresh history.
This separates live Server behavior from already-loaded UI content.

For a redacted read-only inventory/history check on the exact same target:

    uv run contrib/beeper-tools/check_endpoint.py --beeper-target zapfast-headless --label headless --sample-history

It emits only counts, whitelisted network/provider/status labels and a sample
message count, not account IDs, people, chat titles, message text or tokens.
It never starts the Server or reconnects accounts. Optional `--expect-accounts N`
and `--expect-chats N` compare locally recorded baseline counts, exiting2 on mismatch.
Matching counts do not prove per-account identity, full history, message delivery or
bridge persistence. They are a first drift alarm only. Compare before/after results
locally. A history sample covers one chat, not every account's health.

## 4. Persistence and footprint are acceptance gates

Restart only the named trial Server:

    beeper targets restart zapfast-headless

Then inspect account/history availability again on that target. The official
open issue38 reports local Instagram/Facebook disappearing after Server
restart. Readiness/connected alone is not enough. Do not delete/relink accounts
in response; preserve the failure and stop expanding that network trial.

Compare total Server + bridge + ZapFast memory/CPU against the full Desktop
process tree at the same accounts/history/workload after sync settles. A single
ZapFast process snapshot is not a total saving. Disk includes Server DB/media,
bridge state, client cache and build artifacts. The UI's cache cap only controls
its own disposable media, not Server storage.

If the trial fails, quit ZapFast and stop only the trial target:

    beeper targets stop zapfast-headless

Reopen the unchanged Desktop. Do not remove targets/data or undo network
connections blindly. Some network connection flows can affect other sessions;
check their local prompts before confirming.

## Milestones, not a completion promise

1. New Server signed in/verified; first account inventory present.
2. Read-only chat/history refresh works with Desktop quit.
3. Account and history remain after Server restart.
4. Add remaining accounts one at a time, repeating persistence checks.
5. Measure complete settled footprint and finish native/mutation validation.

No guaranteed total setup time: email/device/network approval and sync are
interactive. The source code route is ready to trial; the full mix and memory
saving remain unverified.

Sources rechecked2026-10-28:
https://github.com/beeper/cli
https://github.com/beeper/cli/blob/main/packages/cli/docs/setup.md
https://github.com/beeper/cli/blob/main/packages/cli/docs/targets.md
https://github.com/beeper/cli/blob/main/packages/cli/docs/accounts.md
https://github.com/beeper/cli/issues/38
https://help.beeper.com/en_US/chat-networks/using-on-device-chat-network-connections-in-beeper
