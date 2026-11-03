# Direct-network chats for the read API: design (not built)

Status: design only. Nothing here is implemented, enabled or granted.

## Problem

`zapfast-reader` serves what the Beeper Server serves. fast-beeper's direct
network chats (its own WhatsApp/Signal backends, origin other than
`beeper`) exist only in the running app and its encrypted local archive.
Linked people that include such chats are therefore listed as
`readable: false` and left out of merged timelines.

## Constraints

- The companion must not open or decrypt the app archive, and must not get
  a copy of the archive key.
- No plaintext message snapshot is written to disk.
- Same per-app tokens, scopes, account allowlists and revocation as today.

## Proposed route: a read-only broker inside the running app

1. Off by default. A visible setting ("Allow local apps to read direct
   chats") turns it on; the setting is the user's explicit decision.
2. When on, the app listens on a loopback-only socket (or a unix socket
   with 0600 mode) and accepts exactly one caller class: the reader,
   authenticated with a random secret the app generates into a 0600 file
   (a new secret, unrelated to the archive key). Revoking = deleting the
   file or turning the setting off.
3. The broker answers two read-only questions from memory the app already
   holds: list chats of a direct account, and the newest N messages of a
   chat. Nothing is persisted, nothing is sent, no read receipt or
   read-marker is touched, and requests are bounded like the Server ones.
4. The reader treats broker members as another source behind the same
   scopes and allowlists; `people:read` merged timelines then include them,
   marked by origin, and a missing broker keeps them in `unavailable`.
5. Reversible: removing the setting and the secret returns to today's
   behavior, with no data left behind.

## Why it is not built yet

It needs a listener inside the GUI process and message-model plumbing in a
very large file, and I cannot run or screenshot the app here. The setting
itself is user-visible product behavior. Unverified until built and tried
on a real direct account.

## Alternatives rejected for now

- Reader opens the archive with the app's key: widens the key's reach.
- App exports a plaintext snapshot: persists message content outside the
  encrypted archive.
