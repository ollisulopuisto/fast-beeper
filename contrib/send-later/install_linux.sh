#!/bin/bash
# Install solmu-scheduler as a systemd user service on a Linux Server host
# (the machine that runs Beeper Server). Mirrors install_macos.sh: 0600
# control token, token files never on argv, loopback-only listener.
# Run as the same dedicated user that runs the Server.
set -euo pipefail

CONFIG_DIR="$HOME/.config/zapfast"
BIN_DIR="$HOME/.local/bin"
UNIT_DIR="$HOME/.config/systemd/user"
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

mkdir -p "$CONFIG_DIR" "$BIN_DIR" "$UNIT_DIR"
chmod 700 "$CONFIG_DIR"

# 1. Binary: an explicit path, a built one, else build from this checkout.
if [ -n "${SCHEDULER_BINARY:-}" ]; then
    cp "$SCHEDULER_BINARY" "$BIN_DIR/solmu-scheduler"
elif [ -x "$REPO_ROOT/target/release/solmu-scheduler" ]; then
    cp "$REPO_ROOT/target/release/solmu-scheduler" "$BIN_DIR/solmu-scheduler"
else
    (cd "$REPO_ROOT" && cargo build --release --bin solmu-scheduler)
    cp "$REPO_ROOT/target/release/solmu-scheduler" "$BIN_DIR/solmu-scheduler"
fi
chmod 755 "$BIN_DIR/solmu-scheduler"

# 2. Beeper API token from the environment, never argv.
if [ ! -f "$CONFIG_DIR/beeper-token" ]; then
    if [ -n "${BEEPER_ACCESS_TOKEN:-}" ]; then
        (umask 077; printf '%s' "$BEEPER_ACCESS_TOKEN" > "$CONFIG_DIR/beeper-token")
    else
        echo "Set BEEPER_ACCESS_TOKEN before installing, or write $CONFIG_DIR/beeper-token (mode 0600) yourself."
    fi
fi
[ -f "$CONFIG_DIR/beeper-token" ] && chmod 600 "$CONFIG_DIR/beeper-token"

# 3. Control token, kept if present.
if [ ! -f "$CONFIG_DIR/scheduler-token" ]; then
    (umask 077; openssl rand -hex 32 > "$CONFIG_DIR/scheduler-token")
fi
chmod 600 "$CONFIG_DIR/scheduler-token"

# 4. systemd user unit (the repo's solmu-scheduler.service).
cp "$REPO_ROOT/contrib/send-later/solmu-scheduler.service" "$UNIT_DIR/solmu-scheduler.service"
systemctl --user daemon-reload
systemctl --user enable --now solmu-scheduler
echo "Running. For it to survive logout: loginctl enable-linger \"$USER\" (needs your OK on a shared host)."
echo "Check: systemctl --user status solmu-scheduler"
