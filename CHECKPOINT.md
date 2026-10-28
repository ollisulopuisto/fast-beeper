# ZapFast Beeper development checkpoint, October 28, 2026

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
