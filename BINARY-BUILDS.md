# Manual trial and urgent hotfix binaries

The **Trial binaries** workflow runs only when explicitly dispatched. Choose a
branch/ref and one target (Apple Silicon by default), or deliberately choose `all`.
It does not wait for full CI, and is compile/package evidence, not production
release approval. See [CI-GUIDE.md](CI-GUIDE.md) for check routing and limits.

With uploads off there is no retained download. Download an artifact only after
a separately approved storage decision enables the repository variable
`FAST_BEEPER_UPLOAD_ARTIFACTS=true`. Uploads default off until account storage and
a no-spend budget or permitted storage cost are checked. Compilation and local
packaging run without uploads. Artifacts expire after one day. Each contains a zip and its SHA-256
checksum. The zip includes the compiled binary, launcher, documentation, licenses,
commit/target metadata and per-file checksums. A completed build is not proof of
successful sign-in, sending or OS keychain/restart behavior.

Targets: Apple Silicon Mac (macOS 15 runner), Intel Mac (macOS 15 Intel runner),
Linux x64 (Ubuntu 24.04), Windows x64. Native builds use the real source and pinned
Rust toolchain, not diagnostic stubs. Linux binaries need compatible system
libraries, including PipeWire, ALSA, Wayland/X11 and OpenGL; they are not static
or universal Linux packages. macOS minimum runtime version is not yet established
by testing. Choose the artifact matching your OS and architecture.

## Run a download

Keep the existing Beeper Desktop or Server endpoint running. Extract the inner
zip. From its extracted directory, using Python 3 (or `uv run --no-project python`):

```sh
python contrib/beeper-tools/run_client.py --binary ./zapfast
```

On Windows use `--binary ./zapfast.exe`. This starts read-only; add
`--allow-writes` only when you intend to send and update read state. The launcher
prompts for the existing Beeper API token locally, never in chat. It defaults to
Desktop's `http://127.0.0.1:23373`; use `--base-url` for an existing Server. It does
not install or start Beeper Server. The archive includes no API token or profile.
The launcher's separate prototype profile remains `~/.zapfast-beeper-prototype`.

These are unsigned, unnotarized trial command-line binaries, not installed Mac
apps, signed Windows installers or published releases. macOS Gatekeeper and
Windows SmartScreen may warn or block them. Do not disable OS-wide protections.
Verify the source/commit and checksum before choosing whether to run a download.
Code signing/notarization is a separate setup; no signing credentials are used by
this workflow. Running the bare binary without the Beeper launcher can select the
inherited WhatsApp backend and is not the intended trial path.

## CI limits

Only standard hosted runners; no release publishing, signing services, deployment,
custom images or shared compiler caches. Artifact retention is one day, with
concurrent superseded runs cancelled. Standard-runner compute is free for this
public repository; artifact storage is subject to GitHub's account storage/budget
rules. Recheck billing if repository visibility or runner choice changes.

The inherited tag-triggered Release workflow is separate. This workflow neither
creates tags/releases nor configures the inherited publishing credentials.
