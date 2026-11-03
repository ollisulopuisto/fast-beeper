# Chats the Server does not serve: findings and design (not built)

Status: findings from the code plus a design. Nothing here is implemented,
enabled or granted.

## What the code says

- In Beeper mode every chat the app shows comes from one Beeper API, and
  each chat records that API's base URL as `backend_origin`. People links
  (`beeper.people`) are made only from chats with an origin. So for a user
  running the app against his Server, `zapfast-reader` plus
  `ZAPFAST_READER_ORIGINS` set to the app's recorded URL covers the chats
  AND the linked-person view. No app-owned broker is needed for that.
- A person can still link chats recorded under two different origins (two
  API backends, e.g. a Server and a local Desktop API). One reader serves
  one Server. Serving a second origin means a second upstream in the reader
  (its own token file, configured by the user); not built.
- ZapFast's native WhatsApp mode (no Beeper backend) keeps chats in the
  app's encrypted archive, with no origin, and they cannot be linked
  people. They are not reachable through any Server API.

## If native-mode chats ever need serving: opt-in broker in the app

1. Off by default behind a visible setting; the setting is the user's
   decision.
2. When on, a loopback-only (or 0600 unix) listener inside the running app
   accepts only the reader, authenticated with a fresh random secret in a
   0600 file (unrelated to the archive key). Delete the file or turn the
   setting off to revoke.
3. It answers read-only questions from memory the app already holds. No
   persistence, no receipts, no sends, bounded like the Server routes.
4. The reader treats it as another upstream behind the same scopes,
   allowlists and revocation.

Rejected: the companion opening the archive with the app's key (widens the
key's reach); exporting a plaintext snapshot (persists message content
outside the encrypted archive).

Not built because it needs a listener and a user-visible setting in the GUI
process, which cannot be run or screenshot in the build environment, and
because it only matters for native mode, which this deployment does not use.
