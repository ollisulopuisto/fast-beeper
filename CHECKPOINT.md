# Fast-beeper coordination and development checkpoint

## Active packaging lane - October 29, 2026

Claim: local trial packaging and its verification, owned by the current
fast-beeper packaging contributor. PR6 merged on main e4a2e03. Follow-up PR7
(`fix/verify-mounted-trial-dmg`) adds mounted-image verification and detach
cleanup; it is review/CI work, not a native acceptance or release claim.
Coordinate before changing the same packaging helpers. No new messaging,
backend installation or product-name change is included in this lane.

Repository coordination reviewed: CHECKPOINT.md, QUEUE-1120.md,
CHANGELOG-SCOPE.md, PARITY.md, README-BEEPER.md, CONTRIBUTING.md,
PRODUCT.md, MAC-TRIAL.md, MAC-DMG-TRIAL.md, BINARY-BUILDS.md and the
GitHub contribution/board guidance. No current root AGENTS.md, CODEX.md,
ROADMAP.md, TODO or BUGS ledger is present. The inherited contribution and
product documents describe upstream WhatsApp-only scope; README-BEEPER.md and
this fork's explicit Beeper work describe the current adapter. This does not
claim a direct Matrix implementation or override safety/privacy guidance.

Progress and checks:
- PR5 restored the active source while preserving both repair archive trees.
- Current-source portable checks stage real adapter/model modules, avoiding the
  stale saved harness snapshot. Latest local run:166 Rust and37 Python tests pass.
- Earlier main49ef5e6 native trial compilation/package passed on Apple Silicon,
  Intel Mac and Linux; ordinary Mac/Ubuntu tests passed. This is not proof that
  later commits or an actual DMG/runtime pass.
- PR7 hdiutil mount/content/detach tests are mocked. A real disk image has not
  been created, mounted, visually inspected or launched with this helper.

Blockers and handoff:
- Windows x64 native compilation and ordinary tests failed on49ef5e6. Detailed
  logs were not retrievable; do not invent a root cause from exit codes.
- The Mac Nix job on that head was cancelled during package build, not passed.
- Artifact uploads remain disabled. Do not enable uploads, trigger releases or
  change billing/settings as part of this claim. No downloadable app/DMG exists
  from the trial workflow. Human-led merges only.
- Native DMG, Gatekeeper, launcher, Keychain/restart, existing Server/bridge and
  whole-stack resource acceptance still need separately scoped evidence.
- After PR7 checks/human review, record its actual result here. Further feature
  candidates in CHANGELOG-SCOPE.md are not an automatic implementation claim.

Reproduce current portable checks from the source root:

    ./contrib/beeper-tools/check-source.sh

Direct `cargo test` inside `harness/` tests its saved snapshot, not current app
sources. See MAC-DMG-TRIAL.md for the local trial-image interface and its limits.

## Historical October 28 checkpoint

The following is retained as historical evidence; its test counts and native
compilation statements are superseded by the dated lane above.

Source-only development trial, not an accepted standalone Mac app. Start with
README-BEEPER.md, then MAC-TRIAL.md for the explicit native build command.

## Checks on this source

- Portable adapter/model harness: 163 tests pass, using real adapter/model sources
  and explicit stubs for unrelated WhatsApp/native integrations.
- Python launcher, read-only diagnostic and Mac assembly helper: 26 tests pass.
- Development GUI library and binary build with real egui/UI sources and unrelated
  transport/native-call stubs. Targeted GUI diagnostics and Linux screenshots have
  been inspected; the full inherited GUI suite remains incomplete.
- Portable harness Clippy completes with warnings, not a clean whole-project lint.
- Current older-history regression checks missing/empty/repeated pagination
  cursors, preserving a partial state rather than claiming completeness.

## Native acceptance not performed

Native Mac build/runtime, OS Keychain restart persistence, live Server/network
connections, Desktop-quit/Server-restart behavior, whole-stack CPU/RAM and signing
are NOT RUN. There is no Server installer, sign-in/lifecycle manager or bundled
Server. Server redistribution terms remain unresolved. Unsupported linked outgoing
media and typing stay blocked. Remote search loads one page with partial labeling.

Use a separate prototype profile and an existing exact target only. The launcher
is read-only by default. This checkpoint does not authorize installing a service,
reconnecting accounts, remote exposure or sending real messages. Preserve existing
Beeper Desktop. See BACKEND.md and HEADLESS-SETUP.md for proposed trials and limits.

No binary, credentials, private conversations or overnight development screenshots are included. Inherited upstream demo screenshots and generic assets remain.
The SHA256 archive digest is an integrity check, not a signature.

## Reproduce portable checks (not native acceptance)

    cd harness
    cargo test --locked
    cargo clippy --locked --all-targets
    cd ../contrib/beeper-tools
    uv run --no-project python -m unittest discover -v

Do not mistake the portable harness stubs for the full native app. The top-level
Cargo.toml is the full native source build, described in MAC-TRIAL.md. The GUI
development binary used separate diagnostic dependency stubs and is not included.

History/single-message responses carrying an explicit different chatID are now
rejected before applying content or using media/reaction/confirmation data.
Sparse responses without chatID remain compatible. Local HTTP regression passes.

Control-worker channel closure now stops polling and invokes the queued-upload
stop guard; inflight requests may still finish. Red-green lifetime check passes.
The read-only diagnostic preserves current known account state/provider enums.
Wider inherited tests remain incomplete: library test compilation exceeded the
build window; presence integration tests need native transport APIs absent from
the diagnostic stubs. Portable/targeted tests do not cover those APIs.

Latest App-event recovery diagnostic checks that a late failed send/caption stays
copyable in the existing error list when newer current/parked drafts prevent
restoration. Empty composer restoration still works. Red-green assertions pass;
GUI library/binary build. This is in-session dismissible retention, not a durable
recovery archive: later errors can evict it. No new pixel/native validation.
