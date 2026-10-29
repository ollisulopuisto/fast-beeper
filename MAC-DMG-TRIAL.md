# Local unsigned DMG preparation

This helper prepares a **development-trial disk image**, not a double-clickable
Mac app or an accepted release. It does not install Python, Rust, Beeper Server,
a service, credentials or a profile. It never uploads anything. The existing
trial workflow and its upload gate are unchanged.

The inherited upstream ZapFast.app is not reused: launching that bundle directly
can select the unrelated default WhatsApp backend. This disk image instead
contains the same separate Beeper Python launcher and binary as the portable
trial, with clear backend and credential instructions. Python 3 or an existing
uv installation is required. App-bundle/login/keychain integration remains work
for a separately reviewed native implementation.

## On a Mac

First build the real binary using the existing locked trial helper or Cargo.
For an Apple Silicon build in this source tree:

```sh
cargo build --locked --release --bin zapfast --target aarch64-apple-darwin
```

Choose a version and a **new** output directory. Use the exact commit that
produced the binary. For example, after replacing VERSION and COMMIT_SHA with
reviewed values:

```sh
uv run --no-project python contrib/beeper-tools/package_dmg.py --binary target/aarch64-apple-darwin/release/zapfast --target aarch64-apple-darwin --commit COMMIT_SHA --version VERSION --output dist/NEW_TRIAL_DIRECTORY
```

Intel uses `x86_64-apple-darwin` in both places. The helper checks the executable's
Mach-O CPU/type, rejects an existing output and creates/verifies the image with
macOS hdiutil. It publishes a complete local output directory only after image
verification, with a SHA-256 checksum beside the image. It does not establish
that the supplied commit/version actually built an arbitrary user-supplied binary;
that provenance must be retained by the build caller. No signing, notarization,
minimum macOS compatibility or application runtime acceptance is implied.

## Trying the image

Mount it, inspect the trial instructions, and copy the **whole** trial directory
to a writable local location. Do not run from the read-only mounted image or
launch the bare binary. In the copied directory:

```sh
uv run --no-project python contrib/beeper-tools/run_client.py --binary ./zapfast
```

Use an already running Desktop or explicitly chosen Server endpoint. The launcher
asks for the endpoint's token locally and defaults to read-only. It does not
start, install, sign in or reconnect a Server. See BACKEND.md and HEADLESS-SETUP.md
for the separate operational boundaries. Do not disable OS-wide protections if
macOS blocks this unsigned/unnotarized trial.

## Verification state

The payload, command construction, architecture rejection, non-Mac refusal,
checksum verification, existing-output protection and failed/empty-image cleanup
have portable mocked tests. **A real DMG has not been created, mounted or visually
inspected with this helper yet.** hdiutil execution, Gatekeeper, native launch and
minimum system compatibility require a Mac acceptance run. This document does
not claim a downloadable DMG or a released app.
