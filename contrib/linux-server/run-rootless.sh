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
# --network=host keeps the Server on the host's loopback (127.0.0.1:23374),
# which the Mac reaches only through its own SSH tunnel. It is not published
# on any interface. Dropped capabilities, no extra privileges, no sockets and
# no host paths are mounted.
podman run -d --name zapfast-beeper-server --replace \
  --network=host \
  --cap-drop=ALL --security-opt=no-new-privileges \
  --userns=keep-id:uid=1000,gid=1000 \
  --restart=on-failure \
  -v zapfast-beeper-data:/home/beeper:Z \
  zapfast-beeper-server
mkdir -p "$HOME/.local/bin"
install -m 755 "$here/zapfast-beeper-target" "$HOME/.local/bin/zapfast-beeper-target"
echo "Container started. First time only: see README.md step 3 to sign in."
