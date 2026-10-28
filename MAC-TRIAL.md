# Mac build and standalone-backend acceptance

This is a user-run development trial, not an installed or accepted app. No Mac
build has run in this development environment. Do not run the original upstream
release pipeline or publish to GitHub to try it. Preserve existing Beeper Desktop.

## Local build helper

`rust-toolchain.toml` specifies Rust 1.98.0. Have the Rust toolchain and Xcode
command-line tools installed separately. The helper installs nothing. A build
may download Cargo dependencies and can fail on native dependencies; finding
commands does not prove dependency compatibility.

From the source root, check only:

    uv run --no-project python contrib/beeper-tools/build_macos.py

Explicit local compile/assembly into a new directory:

    uv run --no-project python contrib/beeper-tools/build_macos.py --build --out ./dist/zapfast-local-trial

It chooses arm64 or Intel from the actual Mac, runs a locked release build and
assembles the binary, launcher/diagnostic scripts and backend/trial documentation.
An existing output is refused. The helper explicitly uses this source tree's target directory, so a shell CARGO_TARGET_DIR cannot make it copy an old binary from another location. checksums.json records SHA256 for the assembled files; it is an integrity manifest, not a signature or native acceptance. This is not a universal binary, .app or DMG and is
not verified signed/notarized. No Beeper Server binary or service is included.
The source retains upstream packaging scripts, but they are not acceptance for
this fork and the inherited bundle identity is not an approved release identity.

From the assembled trial directory, use a pre-existing explicitly chosen target:

    uv run --no-project python contrib/beeper-tools/run_client.py --beeper-target zapfast-headless --binary ./zapfast

This launcher is read-only by default. It inspects one target, validates access,
uses an isolated prototype profile and launches the binary. It does not create,
start, authorize or reconnect that target. Without `--binary`, source-tree Cargo
launch is assumed and is not available from a binary-only trial directory.

## Backend trial, separately approved

Read BACKEND.md and HEADLESS-SETUP.md before any setup. Server installation,
sign-in/device verification, network connections, auto-start and remote exposure
are separate operational steps requiring approval. Local Mac Server is the first
proposed target, not a deployment already selected or performed. Do not copy
Desktop databases, reuse its port by guess, remove accounts or log Desktop out.

## Acceptance log to fill on the Mac

Record PASS/FAIL/NOT RUN, date, exact app/CLI/Server version, machine architecture
and generic account/network counts. Never copy tokens, chats, network cookies,
recovery keys or private target JSON into the report.

- [ ] Locked native build completed without stubs; binary architecture checked.
- [ ] Actual UI launched, resize/keyboard/source routing inspected.
- [ ] Prototype state is separate from installed stock ZapFast/Beeper state.
- [ ] Encrypted draft saved, app restarted and same draft restored via Keychain.
- [ ] Exact Server target signed in and encrypted device verified.
- [ ] Intended account inventory and real chat history available read-only.
- [ ] Quit Desktop normally: new history fetch still succeeds through Server.
- [ ] Restart only the trial Server: accounts/history still present per network.
- [ ] Endpoint outage/recovery observed without message loss/automatic resend.
- [ ] Invalid/revoked token fails clearly; correct target reauthorization works.
- [ ] Deliberate sends tested only with approved recipient and reviewed words.
- [ ] Total settled client + Server + bridges CPU/RAM/disk measured fairly.
- [ ] Stop/undo path tested without deleting current connections or state.
- [ ] Packaging/dependency notices/signing reviewed before external distribution.

Known reported Server issue: local Facebook/Instagram may be absent after restart
(https://github.com/beeper/cli/issues/38). Account counts alone cannot prove history
or bridge health. Stop expanding a failed network trial, preserve evidence and
leave the working Desktop setup unchanged. Do not automatically delete/reconnect.
