#!/bin/bash
# Install solmu-scheduler as a launchd agent on macOS.
# - binary into ~/Library/Application Support/zapfast/bin/
# - 256-bit control token in a 0600 file (reused if present)
# - LaunchAgent with KeepAlive, token via env file path, never on argv
set -euo pipefail

APP_SUPPORT="$HOME/Library/Application Support/zapfast"
BIN_DIR="$APP_SUPPORT/bin"
TOKEN_FILE="$APP_SUPPORT/scheduler-token"
CONFIG_DIR="$HOME/.config/zapfast"
PLIST="$HOME/Library/LaunchAgents/com.solmu.scheduler.plist"
LISTEN="127.0.0.1:23376"
BEEPER_URL="${BEEPER_URL:-http://127.0.0.1:23374}"

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

mkdir -p "$BIN_DIR" "$CONFIG_DIR" "$HOME/Library/LaunchAgents"
chmod 700 "$APP_SUPPORT" "$CONFIG_DIR"

# 1. Binary: reuse a built one, else build.
if [ -x "$REPO_ROOT/target/release/solmu-scheduler" ]; then
    cp "$REPO_ROOT/target/release/solmu-scheduler" "$BIN_DIR/solmu-scheduler"
else
    (cd "$REPO_ROOT" && cargo build --release --bin solmu-scheduler)
    cp "$REPO_ROOT/target/release/solmu-scheduler" "$BIN_DIR/solmu-scheduler"
fi
chmod 755 "$BIN_DIR/solmu-scheduler"

# 2. Beeper API token: reuse a stored one, else take it from the
#    environment (BEEPER_ACCESS_TOKEN) so it never lands on argv.
BEEPER_TOKEN_FILE="$APP_SUPPORT/beeper-token"
if [ ! -f "$BEEPER_TOKEN_FILE" ]; then
    if [ -n "${BEEPER_ACCESS_TOKEN:-}" ]; then
        printf '%s' "$BEEPER_ACCESS_TOKEN" > "$BEEPER_TOKEN_FILE"
    else
        echo "Set BEEPER_ACCESS_TOKEN (the local API token) before installing, or"
        echo "write it to $BEEPER_TOKEN_FILE yourself afterwards."
    fi
fi
[ -f "$BEEPER_TOKEN_FILE" ] && chmod 600 "$BEEPER_TOKEN_FILE"

# 3. Control token: keep an existing one, else generate 256 bits.
if [ ! -f "$TOKEN_FILE" ]; then
    openssl rand -hex 32 > "$TOKEN_FILE"
fi
chmod 600 "$TOKEN_FILE"
# Solmu reads the token from its own config dir; link it there.
ln -sf "$TOKEN_FILE" "$CONFIG_DIR/scheduler-token"
chmod 600 "$CONFIG_DIR/scheduler-token" 2>/dev/null || true

# 4. LaunchAgent. The token goes via SOLMU_CONTROL_TOKEN_FILE so it is
#    never in argv or the plist.
cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.solmu.scheduler</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN_DIR/solmu-scheduler</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>SOLMU_LISTEN</key>
        <string>$LISTEN</string>
        <key>BEEPER_BASE_URL</key>
        <string>$BEEPER_URL</string>
        <key>BEEPER_ACCESS_TOKEN_FILE</key>
        <string>$APP_SUPPORT/beeper-token</string>
        <key>SOLMU_CONTROL_TOKEN_FILE</key>
        <string>$TOKEN_FILE</string>
        <key>SOLMU_QUEUE_PATH</key>
        <string>$APP_SUPPORT/queue.json</string>
    </dict>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$APP_SUPPORT/scheduler.log</string>
    <key>StandardErrorPath</key>
    <string>$APP_SUPPORT/scheduler.log</string>
</dict>
</plist>
PLIST

# 4. Load (reload if already loaded).
launchctl unload "$PLIST" 2>/dev/null || true
launchctl load "$PLIST"

echo "solmu-scheduler installed and loaded."
echo "  socket: $LISTEN (bearer-protected)"
echo "  token:  $TOKEN_FILE (0600)"
echo "  queue:  $APP_SUPPORT/queue.json"
echo "Solmu will pick the token up from $CONFIG_DIR/scheduler-token."
