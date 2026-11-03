# fast-beeper read API (EXPERIMENTAL, local only, not ready)

`zapfast-reader` lets other apps on the same machine read messages through
fast-beeper with their own revocable credentials. It runs beside the Beeper
Server, like `zapfast-scheduler`. This is code and docs only: nothing here
is installed, deployed, or granted to any app.

## What exists and what it adds

- Beeper's own Server/Desktop API needs a token on every call, but a token
  is all-or-nothing and can send messages. Handing it to another app is not
  least privilege. It is left alone and never exposed.
- The Send Later daemon has one shared control token and can queue sends.
  It is not reused here.
- `zapfast-reader` holds the Server token itself and offers only GET on four
  endpoint families. No send, edit, delete, read-marker, reaction or
  focus endpoint is routable (tests assert this).

## Endpoints

| Scope | Endpoint |
|---|---|
| `accounts:read` | `GET /v1/accounts` |
| `chats:read` | `GET /v1/chats` |
| `messages:read` | `GET /v1/chats/{id}/messages` |
| `search:read` | `GET /v1/messages/search?query=` |
| `people:read` | `GET /v1/people`, `GET /v1/people/{id}/messages?limit=` |

## Linked people (fast-beeper's own view)

Set `ZAPFAST_SETTINGS_FILE` to the app's `settings.json` (read-only; only
`beeper.people` is read, nothing else in the file is parsed into a
response). `/v1/people` lists linked persons: a content-derived `id`, name,
primary destination and members, each marked `readable`. `/v1/people/{id}/messages`
merges the newest messages across the person's readable members (limit
capped at 100, no paging), adds `chatID`/`accountID` to each item, and lists
every member it could not read under `unavailable` with `partial: true`.
- Members from the app's direct network backends (origin other than
  `beeper`) are stored in the app, not the Server. They are listed as
  `readable: false` and are NOT in the merged timeline. Reading them needs
  a separate, app-owned source and is not built.
- Each readable member's chat is checked against the Server (it must exist
  and belong to the recorded account) before its messages are read.
- A restricted client sees a person only if every member is inside its
  account allowlist, so a cross-account link is never revealed.
- Without `ZAPFAST_SETTINGS_FILE`, or if the file is unreadable or over
  4 MiB, people endpoints return 502.

Query keys are limited to `cursor`, `direction` (before/after), `limit`
(capped at 100) and, for search, `query`. Anything else is refused with 400.
Responses come from the Server unchanged for unrestricted clients.

## Authentication and authorization

- One named token per app: `zapfast-reader token add NAME --scopes a,b
  [--accounts x,y] [--days N]`. The token prints once on the terminal.
  Only its SHA-256 is stored, in `~/.config/zapfast/reader-clients.json`
  (mode 0600 from creation; a looser file is refused on load).
- `token list` shows names, scopes, accounts and expiry. `token revoke NAME`
  removes the entry. The file is re-read per request, so revocation is
  immediate, no restart.
- `--accounts` restricts a client to those Beeper account ids. Listings and
  search are filtered by account; everything except the filtered `items`
  (counts, cursors, hasMore, lookup maps) is dropped, so restricted clients
  get one page and no paging; a chat's
  messages are served only if the chat belongs to an allowed account, and
  otherwise look like 404. An unexpected response shape fails closed (502).
  Chat-level allowlists are not supported: a registry entry with unknown
  fields is rejected, and a bad registry refuses all requests.
- Constant-time hash comparison, 120 requests per minute per token,
  loopback bind only (non-loopback addresses refuse to start), requests
  with Origin or Referer headers refused, 5 s read timeout, 16 KiB request
  cap, `cache-control: no-store`, upstream error bodies never forwarded.
- Log lines hold client name, endpoint family and status. Never query
  strings, chat ids or message content. The token CLI never reads the
  upstream token.

## Not covered (explicitly)

- Direct-network (non-Beeper) chats, and therefore the direct-network part
  of any linked person, are not served: they live in the app's own storage.
  Reading them is the main remaining coverage work.
- The scheduled queue is not exposed through this API.
- No push or websocket: clients poll.
- No remote access. Reaching it from another machine (SSH tunnel, VPN) is a
  separate, later decision.
- The server is single-threaded; a slow local client can delay others up to
  the 5 s read timeout.

## Unverified

- Whether the Beeper Server marks a chat read when its messages are fetched
  with GET. The Server docs do not say. This API never calls the read or
  unread endpoints, but the upstream behavior needs a live check before the
  API is described as side-effect free.
- Live response shapes (`items`, `accountID` on chats and search results)
  are taken from the app's own parsing. Restricted clients depend on them
  and fail closed if they differ.
- Behavior with a real Server and real client app: untested.

## Run (manual)

    export BEEPER_ACCESS_TOKEN_FILE=~/.config/zapfast/server-token
    zapfast-reader token add dashboard --scopes chats:read,messages:read
    zapfast-reader                       # serves 127.0.0.1:23377
    curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:23377/v1/chats

## Open security work before this can be called ready

- HTTP-level tests (tests/read_api_http.rs) cover auth, Origin/Referer,
  traversal, probe, redirect, error bodies, revocation, rate limit,
  connection cap and registry permissions against a mock Server. They have
  not been run against a real Server.
- Each connection has its own thread (cap 32) and its own upstream handle,
  so a slow Server call delays only its own connection (up to 30 s per
  call, several calls for a merged timeline).
- `token add/revoke/list` take an exclusive lock file; a lock older than
  120 s is treated as a crash and removed.
- Live check of Server side effects (see Unverified) and of response shapes.
- Direct-network chats and their messages: not built (active missing work).
- Linked-person endpoints are tested only against synthetic settings and a
  mock Server.
