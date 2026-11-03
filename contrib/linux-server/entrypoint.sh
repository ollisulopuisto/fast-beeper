#!/bin/sh
# Container entry: start the Server target once it exists, then stay alive
# by following its log so the container stops if the Server exits.
set -eu
if beeper targets list 2>/dev/null | grep -q '\bserver\b'; then
  beeper targets start server
  exec beeper targets logs server --follow
fi
echo "No 'server' target yet. Run the first-time setup from contrib/linux-server/README.md." >&2
exec sleep infinity
