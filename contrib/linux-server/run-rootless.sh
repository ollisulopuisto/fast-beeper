#!/bin/sh
# Run the Beeper Server container as the CURRENT user. Use a dedicated,
# unprivileged account (no sudo, no CI runner, home mode 0700), never a
# shared account. Nothing is installed, enabled or opened by this script
# beyond the container itself; sessions and logins stay in the data volume.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
[ "$(id -u)" -ne 0 ] || { echo "Run as the dedicated non-root user." >&2; exit 1; }
podman build -t zapfast-beeper-server "$here"
podman volume exists zapfast-beeper-data || podman volume create zapfast-beeper-data
# Private rootless network (pasta). The Server listens on 127.0.0.1 inside the
# container, and the CLI offers no other bind. --host-lo-to-ns-lo makes pasta
# deliver a forward from the host's loopback to the container's loopback
# (passt.1: https://passt.top/builds/latest/web/passt.1.html). Without it,
# forwards go to the container's public address and a loopback-only Server is
# unreachable. The port is published on the host's 127.0.0.1 only. Needs
# Podman 5 or a pasta that accepts the option. Untested: if it fails, do not
# fall back to --network=host silently (see README).
podman run -d --name zapfast-beeper-server --replace \
  --network=pasta:--host-lo-to-ns-lo \
  -p 127.0.0.1:23374:23374 \
  --cap-drop=ALL --security-opt=no-new-privileges \
  --userns=keep-id:uid=1000,gid=1000 \
  --restart=on-failure \
  -v zapfast-beeper-data:/home/beeper:Z \
  zapfast-beeper-server
mkdir -p "$HOME/.local/bin"
install -m 755 "$here/zapfast-beeper-target" "$HOME/.local/bin/zapfast-beeper-target"
echo "Container started. First time only: see README.md step 3 to sign in."
