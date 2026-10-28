# Backend package and deployment status

Checked October 28, 2026. This source package is not a turnkey installed Mac app.
It contains a Beeper Client API adapter, local launcher, read-only diagnostics,
mock, tests and proposed deployment instructions. A Mac-only local build/assembly helper and acceptance log are in MAC-TRIAL.md.
It does not contain or install a Beeper Server binary. No setup/reconnection is authorized by this document.

## What runs where

| Route | On the Mac | Backend host | Can Desktop quit? | Prototype status |
| --- | --- | --- | --- | --- |
| Existing Desktop API | ZapFast + Beeper Desktop | Same Mac | No: API lives in Desktop | Adapter/fixtures; live Mac unverified |
| Standalone same-Mac Server | ZapFast + Beeper Server and its bridges | Same Mac | Yes in upstream architecture; independently signed-in Server required | Proposed first acceptance target, not installed/tested |
| Remote Server | ZapFast | Another supported machine running Server/bridges | Yes; remote Server must remain running/reachable | HTTPS/token configuration implemented; live remote unverified |
| Remote Desktop | ZapFast | Another machine running Desktop | Only on client Mac, not backend host | Same remote caveats |
| Direct Beeper Cloud | ZapFast only | Beeper-managed bridge service | Not an implemented direct API route | No direct-cloud client or Matrix transport implemented |

A Beeper Cloud connection is a network-bridge choice, not a substitute API URL.
This client still needs a running Desktop or Server endpoint. Cloud connections
follow the Beeper account. On-device connections are set up separately on each
runtime/device and support multiple accounts per network. Official help currently
says Cloud supports one account per network. A separate Server must not be assumed
to inherit Desktop's on-device logins, history or permissions.

The CLI's self-hosted Server is distinct from `bbctl` bridge self-hosting. Running
only a bridge manager does not supply this client's Beeper Client API. iMessage is
macOS-only in the current Client API documentation; a generic Linux host must not
be promised to replace the Mac requirement. Exact Server/iMessage support and OS
permissions must be checked on the chosen version, not inferred from archived
bridge projects.

## Server distribution boundary

The official CLI resolves a platform/channel Server artifact from Beeper's update
feed, installs it in its own managed location and tracks its version. We do not
invent or pin a static download URL. The CLI's MIT license is verified, but it
must not be assumed to license the separately downloaded Server binary. Runtime
redistribution terms are unresolved here. The local-development path therefore
uses the official CLI as a separately installed dependency, not an unlicensed
copy of Server bundled into this package. A distributable turnkey installer
requires resolving runtime terms/version/update/signature policy first.

Sources: https://github.com/beeper/cli/blob/main/packages/cli/src/lib/installations.ts
and https://github.com/beeper/cli/blob/main/packages/cli/LICENSE . Archived
`beeper/self-host` and individual bridge licenses do not settle Server's terms.

## Authentication and permissions

There are separate layers:

1. Beeper session sign-in, with browser OAuth or emailed code for headless setup.
2. Encrypted-device verification: approve from another signed-in device or use
   a recovery key locally. Recovery keys are not ordinary API access tokens.
3. Each network's connection/login, with its own QR/code/browser/cookie flow and
   device limits. Reconnection may affect other sessions. It requires explicit
   approval, not an automatic response to an empty chat list.
4. ZapFast's access to the chosen Client API using that endpoint's bearer token.

Upstream CLI supports OAuth/PKCE and stores target authentication. This prototype
reads the exact named target URL/token once at launch or accepts an explicit URL
and locally entered token. It has no in-app OAuth flow or token renewal. If access
expires/is revoked, reauthorize the correct target locally and relaunch. Do not
paste target config, recovery keys, API tokens or network cookies into chat.

The launcher defaults to read-only; deliberate write support requires its explicit
`--allow-writes` switch. This is a client-side safety guard, not a server-enforced
restricted credential. A token must still be treated as sensitive. The launcher
uses a separate `~/.zapfast-beeper-prototype` profile, does not change the CLI's
default target and does not install, start, stop or reconnect anything.

## Remote operation

Remote hosting needs a persistent backend, network reachability and a secured
endpoint. Beeper documents a localhost default and an explicit remote-access
setting that broadens listening to network interfaces. Exposing it can expose
history and permit sends. Beeper supplies no tunnel service. Do not enable remote
access, install a tunnel, open firewall ports or rent a server as part of merely
running this source package.

The prototype accepts remote HTTPS and refuses remote plain HTTP. Local loopback
HTTP is allowed, bypasses environment proxies and redirects never receive a
forwarded bearer token. CLI target selection and valid TLS do not prove correct
operator identity, least-privilege access, backup policy or secure hosting. These
remain deployment decisions. VPN/tunnel configuration is not included or tested.

## Recovery actually implemented

- Startup retries account reads while endpoint is unavailable. Text/edit/file
  requests during that window return retained content with an explicit failure,
  not a delayed automatic write. Other actions report not performed.
- Open/linked history polls every 3 seconds; inventory refresh every 60 seconds.
- Network failure is not repaired by account removal/relinking.
- History failures can be retried independently for each linked member.
- Ambiguous send outcomes retain text and say to check history before retrying;
  no blind automatic resend. Tokens are not renewed by polling.
- Client does not supervise a Server process, wake a sleeping host, repair a
  bridge's login or restore a lost database.

For production, upstream CLI supports managed start/stop/restart and start-at-login:
launchd on macOS, systemd user units on Linux. Such service installation is not
performed here. Mac sleep/logout/reboot and remote-host failure are still practical
availability concerns. Persistent storage and secure backups need an explicit
plan for Server state, bridge state, media and client drafts.

## Proposed acceptance, not completed work

1. Build/run native Mac client; verify Keychain and isolated profile behavior.
2. With approval, create/reuse one separate named Server target. Preserve Desktop.
3. Sign in/verify; inspect existing inventory. Connect at most one approved test
   account if needed. Do not migrate or duplicate all accounts at once.
4. Read-only real chat/history check while Desktop is quit; distinguish fetched
   data from already-loaded content. Use HEADLESS-SETUP.md for proposed commands.
5. Restart only that trial Server. Compare account counts and actual history.
   Upstream issue38 is still open: a Linux/Docker report says local Facebook and
   Instagram accounts/chats vanish after restart. It is not proof every setup
   fails, but green readiness alone is insufficient.
6. Test disconnect/recovery, token expiry and no duplicate sends. Only then approve
   a deliberate send to an agreed test recipient with reviewed text.
7. Add remaining approved accounts one at a time and repeat persistence checks.
8. Compare total settled Server + bridge + ZapFast CPU/RAM/disk against Desktop
   with equivalent accounts/history/workload. No whole-stack savings claim yet.
9. Finish installer/runtime dependency distribution, version compatibility,
   licensing/notices, service setup/undo and native signature/notarization work
   before calling this an end-to-end deliverable.

## Evidence and current gaps

Portable adapter/model suite: 163 tests. Launcher/redacted diagnostics/build-helper: 26 tests.
Recovery fixture covers startup503, endpoint recovery, a deliberate failing
post-connect send and recovered history without write replay.
These use local fixture HTTP and stubs for unrelated native/archive code. Linux
GUI fixtures and targeted real GUI-library assertions exercise actual UI logic.
None is a native Mac, live bridge, Server install, Desktop-quit or restart test.
No bundled Server/service, in-app auth/onboarding, automatic token refresh,
server backup/restore, verified Mac build or operational whole-stack acceptance
is complete. See PARITY.md for client limitations, including linked text-only sends.

## Sources read for this checkpoint

- Desktop runtime/local API and macOS-only iMessage: https://developers.beeper.com/desktop-api
- Server/remote/token options: https://github.com/beeper/cli/
- Headless/email/device verification/service persistence: https://github.com/beeper/cli/blob/main/packages/cli/docs/setup.md
- Explicit targets and lifecycle: https://github.com/beeper/cli/blob/main/packages/cli/docs/targets.md
- Token source and encrypted-device auth: https://github.com/beeper/cli/blob/main/packages/cli/docs/auth.md
- Network sign-in/reconnect flows: https://github.com/beeper/cli/blob/main/packages/cli/docs/accounts.md
- Cloud/on-device and multi-account limits: https://help.beeper.com/en_US/chat-networks/using-on-device-chat-network-connections-in-beeper
- Remote exposure warning: https://developers.beeper.com/desktop-api/advanced/remote-access
- Bridge manager, a different component: https://developers.beeper.com/bridges/self-hosting/
- Current reported restart bug: https://github.com/beeper/cli/issues/38

Adapter configuration also rejects root URL paths/query/fragments when launched directly. Decode and transport errors avoid response values and chat paths. Chat inventory is capped at 100000 rows and 1000 pages, including empty advancing pages; reaching a cap is an explicit error, not a partial successful inventory.

The same bounded 16 MiB JSON reader now covers send and upload confirmations, not only reads. An oversized or unreadable send confirmation reports an uncertain outcome and asks to check history; it never automatically retries the send.

Connection screens name the Beeper API rather than WhatsApp. Before the first inventory, unavailable endpoints show a wrapping explanation instead of an empty chat list. Beeper configuration failures require correcting the existing local setup and relaunching; a misleading nonfunctional Retry button is not shown. Account authorization is not renewed in-app.

Live history polling failures now have per-chat warnings (including linked members); successful polling clears only the live warning, never a separate failed historical-page warning. Loaded content stays visible. Polling yields after 32 advancing pages per chat and continues from its cursor next cycle, rather than monopolizing the control worker with unbounded catch-up.

When a live continuation claims more pages but omits or repeats its cursor, the client preserves its last good cursor and shows a gap warning, rather than silently treating the chat as current. Incoming polling moves the loaded page into the UI event and only clones incoming messages needed by the separate notification event.

A failed account inventory refresh retains the last account labels but explicitly marks them as potentially stale, even if chats refresh successfully. Pending confirmations that remain PENDING beyond the 60-second observation window now show an unconfirmed-delivery warning; the client neither claims failure nor resends them.

Shutdown sets a shared upload stop flag on every control-worker exit. Upload workers check it before each queued file, before asset upload and after asset confirmation before message submission. This prevents normal queued-batch draining after shutdown. It cannot recall a request already in flight or make the final flag-check/request boundary atomic; any such request still needs history confirmation. Detached read workers can finish an existing bounded-time request after shutdown.

Global message search now reports query-scoped failures instead of silently showing no matches, and flags results as partial when the first API page has more results. Only one search page is loaded. Native per-chat search now calls the API with an exact chat filter and optional day range, checks chat/date scope locally, and reports errors instead of leaving the pane pending. It loads one API page and honors its partial-results flag. In a linked view this pane searches the selected member only, explicitly labeled; the linked timeline search covers all loaded sources.

Unsupported pasted-image, voice and location sends return their payload to the existing local refusal/recovery path instead of discarding it after a generic notice. This applies during startup retry and normal connected handling. These content types remain unsupported, not newly sendable.

Older-history pages that claim more results must carry a nonempty advancing cursor. Missing/repeated cursors now leave history explicitly partial, retain valid prior cursors, and do not silently refetch the latest page as older history. Retry after service recovery is manual; this is not proof of exhaustive real-network history.

History and single-message lookups reject an explicitly different response chatID before applying content to the requested conversation. A sparse response with no chatID remains compatible. A generic local HTTP test covers this guard; it is not evidence of a crossed response from a live Beeper server.

The connected control worker now exits when the UI's command channel closes, even without an explicit Shutdown. Previously it continued background polling. The existing upload-stop guard runs on this exit as well; already-started requests and bounded detached read workers can still finish. A local worker-lifetime regression covers channel closure, not full native process supervision. The redacted inventory diagnostic preserves current known account-state/provider enums, including attention/reconnect requirements and self-hosted/platform-sdk, without exposing arbitrary values.

Late Beeper send failures no longer silently skip old text when a newer current or parked draft blocks restoration. The newer draft stays unchanged; old text/captions appear in the existing copyable error. Empty composers still restore normally. This is in-session, dismissible error retention, not a durable recovery archive: later errors can evict it. A real App-event diagnostic checks both cases; native/pixel validation remains unperformed.
