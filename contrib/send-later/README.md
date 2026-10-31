# Send Later: scheduler service setup

`zapfast-scheduler` is a small companion daemon that owns the durable
Send Later queue. fast-beeper talks to it over a localhost socket; the
daemon talks to the Beeper API. Because the daemon holds the queue on
disk, scheduled messages go out even while fast-beeper is closed.

Pieces:

- `zapfast-scheduler` - the daemon binary (built from this repo).
- A **control token**: a random secret the app and the daemon share,
  stored in a 0600 file. The daemon refuses to start without one and
  refuses any request that does not bear it. The token is never written
  into settings, logs, or the repository.
- A **launchd agent** (macOS) that keeps the daemon running.
- A **managed SSH tunnel** when the Beeper server runs on another
  machine (the Studio deploy).

## Local install (daemon on the same Mac as fast-beeper)

```
./contrib/send-later/install_macos.sh
```

The script:

1. builds `zapfast-scheduler` (or copies a prebuilt one from
   `target/release/` if present) into `~/Library/Application Support/zapfast/bin/`;
2. generates a 256-bit control token into
   `~/Library/Application Support/zapfast/scheduler-token` with mode 0600
   (reusing an existing token if one is already there);
3. writes `~/Library/LaunchAgents/com.zapfast.scheduler.plist`
   (KeepAlive, run at login, token passed via `ZAPFAST_CONTROL_TOKEN_FILE`,
   never on the command line or in the plist);
4. loads the agent.

fast-beeper reads the token from the same file
(`~/.config/zapfast/scheduler-token`, symlinked by the installer into the
app's config dir when it differs). Set `Settings -> scheduler_url` only
if you moved the socket off the default `http://127.0.0.1:23376`.

## Studio deploy (Beeper server on another machine)

The daemon should run **next to the Beeper server**, so message delivery
does not depend on the MacBook being awake. fast-beeper then reaches the
daemon through a managed SSH tunnel, the same pattern the Beeper API
tunnel (`127.0.0.1:23374`) already uses:

1. Install the daemon on the Studio machine (macOS: the same
   `install_macos.sh` / launchd agent applies; `zapfast-scheduler.service`
   is the systemd user unit for a Linux host instead).
2. Give the MacBook a tunnel: `ssh -N -L 23376:127.0.0.1:23376 dst@192.168.1.222`,
   managed by the same mechanism that maintains the API tunnel.
3. Copy the token file to the MacBook's config dir (0600); both sides
   must hold the same secret.

The token travels the SSH tunnel only; the socket never leaves
localhost on either machine.

Actual Studio changes (installing the binary, creating the user unit,
extending the tunnel config) are a separate deploy step with its own
permission - this directory only prepares the artifacts.

## Queue durability and safety

- The queue lives in `queue.json` next to the token file, saved
  atomically (temp file + rename) after every change.
- A send attempt that ends without an answer (timeout, crash) becomes
  `uncertain` and is NEVER resent automatically - reconcile in the chat
  and retry by hand.
- A message due while the daemon was down becomes `missed`; it needs an
  explicit retry within the 15-minute grace window or it stays parked.
- Enqueue validates the chat and account against the Beeper API, so a
  message cannot be queued into a chat the account cannot send to.
