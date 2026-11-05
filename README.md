# Solmu

A fast, native macOS client for [Beeper](https://www.beeper.com). It pairs
the egui interface from [ZapFast](https://github.com/crmne/zapfast) with a
Beeper Client API backend instead of a direct WhatsApp connection, so chats
from every network connected to your Beeper account live in one lightweight
native window. No browser engine.

This is a personal tester build, not a product release: the app is unsigned
(ad-hoc signed, not notarized), targets Apple Silicon (arm64, macOS 11+),
and is distributed as versioned DMGs on this repository's
[Releases](https://github.com/ollisulopuisto/solmu/releases) page.

## How it works

```
Solmu.app (your Mac)
        |  SSH tunnel, fixed command, loopback only
        v
127.0.0.1:23374 - Beeper Server API (your Server Mac)
        |  Beeper bridges
        v
WhatsApp and your other connected networks
```

The Beeper Server runs on a second Mac (installed with the official Beeper
CLI). The app never talks to a public interface: it opens an SSH tunnel to
the Server Mac and reaches the Server API on loopback port 23374. A Linux
Server host is prepared but not yet in use - see
[contrib/linux-server/README.md](contrib/linux-server/README.md). iMessage
is not part of the Linux setup.

## Install

1. Download `Solmu-Beeper-<version>-<commit>.dmg` from the latest
   [release](https://github.com/ollisulopuisto/solmu/releases) and
   check it against the attached `.sha256`.
2. Drag **Solmu.app** to Applications. Because the build is
   unsigned, open it the first time with right-click > Open.

## First run

The Connect window asks for two things only:

- the Server Mac's SSH hostname or Tailscale IP, and
- your SSH username on it.

Existing SSH key trust between the two Macs is required. The app never asks
for or saves an SSH password, installs nothing on the Server Mac without
your approval, and does not link or reset any account. Close any old manual
tunnel using port 23374 before connecting.

The client starts read-only. Choose write mode explicitly when you want to
send. Your Beeper access token is stored in the macOS keychain; no tokens
live in this repository.

## What you get

- Chats from every network connected to your Beeper account, including
  multiple accounts on one network.
- Chat tabs: keep several conversations open and switch between them.
- Replies with exact-source quoting, avatars, unread badges, and search over
  loaded history.
- Send Later: schedule messages; a small `solmu-scheduler` daemon on the
  Server Mac owns the durable queue, so scheduled messages go out even while
  the app is closed. The app offers a one-time, approval-gated install;
  details in [contrib/send-later/README.md](contrib/send-later/README.md).

Feature-by-feature status and known gaps live in [PARITY.md](PARITY.md);
treat it as a living ledger that can lag development.

## Build from source

Requires the pinned Rust toolchain (1.98.0, see `rust-toolchain.toml`) and a
macOS build host for the app bundle:

```sh
cargo build --locked --release --bin solmu
cargo build --locked --release --bin solmu-scheduler
python contrib/beeper-tools/package_app_dmg.py \
  --binary target/aarch64-apple-darwin/release/solmu \
  --scheduler-binary target/aarch64-apple-darwin/release/solmu-scheduler \
  --output app-dist --version <version> --commit <full-commit-sha>
```

For interface work without a Server, the mock speaks the same API with
generic data:

```sh
uv run contrib/beeper-mock/mock_server.py
SOLMU_BACKEND=beeper BEEPER_DESKTOP_BASE_URL=http://127.0.0.1:8077 \
  BEEPER_ACCESS_TOKEN=mock cargo run
```

`./contrib/beeper-tools/check-source.sh` stages the current adapter/model
sources into a portable harness and runs its checks; CI runs the full test
suite and an explicit on-demand trial build (`trial-binaries` workflow)
packages the unsigned DMG. None of these replace native on-Mac acceptance.

## More docs

- [BACKEND.md](BACKEND.md) - backend routes and deployment status.
- [HEADLESS-SETUP.md](HEADLESS-SETUP.md) - a separate read-only Server trial.
- [MAC-TRIAL.md](MAC-TRIAL.md) - Mac build/assembly helper and acceptance log.
- [contrib/linux-server/README.md](contrib/linux-server/README.md) - moving
  the Server to a Linux host (prepared, not run).
- [README-BEEPER.md](README-BEEPER.md) - the original backend-swap checkpoint.

## License

MIT, as upstream ZapFast. See [LICENSE](LICENSE) and
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
