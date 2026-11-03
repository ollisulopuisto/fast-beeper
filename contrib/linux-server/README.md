# Beeper Server on a Linux host

Status: prepared, **not run on any host**. Nothing here moves sessions,
signs in, opens a port or touches a VPS. The Mac app still reaches the
Server through its own SSH tunnel to `127.0.0.1:23374`, over the tailnet.
The API is never bound to a public interface and Beeper's "Remote Access"
setting is not used.

## What was checked

- Beeper's update feed serves a stable Server artifact for `linux` on both
  `x64` and `arm64` (version 4.3.264 on 2026-11-03):
  `https://api.beeper.com/desktop/update-feed.json?bundleID=com.automattic.beeper.server&platform=linux&channel=stable&arch=arm64`
  (and `arch=x64`). The official CLI installs it (`beeper setup --server --install`).
- Not checked: that the Server starts on a given distro, that each network
  connector works on Linux, and that logins from a datacenter IP are accepted
  (Instagram, Facebook and LinkedIn may challenge them). Try one network first.
- iMessage is not part of this setup.

## Choose the isolation first

Rootless Podman protects the host from the container. It does not protect the
container's data from other processes running as the same user. Use a
**dedicated unprivileged user** (no sudo, no CI runner, home mode 0700) or a
separate VM. Never mount a Podman/Docker socket or host paths into it.

## A. Dedicated user, no container (the CLI's documented path)

As that user: `beeper setup --server --install`, then sign in with your
existing Beeper email (code by email; approve verification from an
already-signed-in client), then `beeper targets enable server` (systemd user
unit) and `loginctl enable-linger <user>`.

## B. Rootless Podman (experimental)

1. As the dedicated user: `contrib/linux-server/run-rootless.sh`.
2. First time only: `podman exec -it zapfast-beeper-server beeper targets add server --port 23374`, then
   `beeper setup --server --install` the same way, and sign in as in A. The app only accepts port 23374. Then `podman restart zapfast-beeper-server`.
3. The script installs `~/.local/bin/zapfast-beeper-target`; the app's Connect
   runs it over SSH (fixed command, loopback 23374 only).

Network: the container uses a private rootless network and publishes only
`127.0.0.1:23374`. The CLI documents a port flag but no bind address, and
reports `http://127.0.0.1:<port>`, so the Server is assumed to listen on
loopback inside the container. pasta forwards host loopback to container
loopback only with `--host-lo-to-ns-lo`; the script sets it. Host networking
is deliberately not used. If the forward does not work on your Podman, host
networking would be the only workaround: it puts the Server in the host's
network namespace (still loopback-bound, but other host users can reach the
port, which the token alone guards). Prefer option A over that.

Untested assumptions: the pasta forward works as described, and the CLI can
start and supervise the Server inside a container without systemd. If either
fails, use A.

## Mac app

Connect with the VPS's Tailscale name and the dedicated SSH user, as for the
Mac Studio. The launcher accepts a Linux host. Send Later is not offered
in-app for Linux (the DMG bundles only a macOS scheduler). Install it on the
host with `contrib/send-later/install_linux.sh` (needs `cargo`, or
`SCHEDULER_BINARY=` pointing to a Linux build). It listens on 127.0.0.1:23376
and writes its control token to `~/.config/zapfast/scheduler-token`. The app's
Connect already forwards 23376 and reads the token from that path, so no
launcher change is needed for it. The trial workflow now builds the scheduler
and runs the stub-Server end-to-end test for x86_64 Linux. arm64 Linux builds
from source on the host.

## Limits

- The disk fills with media and history; set a size budget.
- Read-only mode in fast-beeper is a client guard, not server protection.
  The Server's token and sessions are as sensitive as your messages.
